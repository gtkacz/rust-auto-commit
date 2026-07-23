use crate::config::AppConfig;
use anyhow::Result;
use regex_lite::Regex;
use std::sync::OnceLock;

const CONVENTIONAL_COMMIT_SPEC: &str = "\
Write all commit messages strictly following the Conventional Commits specification.

Use the following format:
<type>[optional scope][optional !]: <description>

[optional body]

[optional footer(s)]

Rules to follow:
1. Type: MUST be a noun. Use `feat` for new features, `fix` for bug fixes, or other relevant types (e.g., `docs`, `chore`, `refactor`).
2. Scope: OPTIONAL. A noun describing the affected section of the codebase, enclosed in parentheses (e.g., `fix(parser):`).
3. Description: REQUIRED. A concise summary immediately following the type/scope, colon, and space.
4. Body: OPTIONAL. Provide additional context. MUST begin one blank line after the description.
5. Footer: OPTIONAL. MUST begin one blank line after the body. Use token-value pairs (e.g., `Reviewed-by: Name`). Token words must be hyphenated.
6. Breaking Changes: MUST be indicated by either an exclamation mark `!` immediately before the colon (e.g., `feat!:`) OR an uppercase `BREAKING CHANGE: <description>` in the footer.";

const GITMOJI_UNICODE_SPEC: &str = "\
Use Gitmoji while still following the Conventional Commits specification above: \
prepend a relevant emoji in unicode format, then a space, then the conventional type(scope): description. \
Examples: \u{26a1}\u{fe0f} feat(api): improve response time, \u{1f41b} fix(auth): correct login redirect, \
\u{2728} feat: add new feature, \u{267b}\u{fe0f} refactor(parser): simplify logic, \u{1f4dd} docs: update README, \u{1f3a8} style(ui): improve layout";

const GITMOJI_SHORTCODE_SPEC: &str = "\
Use Gitmoji while still following the Conventional Commits specification above: \
prepend a relevant emoji in :shortcode: format, then a space, then the conventional type(scope): description. \
Examples: :zap: feat(api): improve response time, :bug: fix(auth): correct login redirect, \
:sparkles: feat: add new feature, :recycle: refactor(parser): simplify logic, :memo: docs: update README, :art: style(ui): improve layout";

/// Build the full system prompt from config flags
pub fn build_system_prompt(cfg: &AppConfig) -> String {
    let mut parts = Vec::new();

    // Base prompt (user-overridable)
    parts.push(cfg.llm_system_prompt.clone());

    // Conventional commits
    parts.push(CONVENTIONAL_COMMIT_SPEC.to_string());

    // Gitmoji
    if cfg.use_gitmoji {
        let spec = match cfg.gitmoji_format.as_str() {
            "shortcode" => GITMOJI_SHORTCODE_SPEC,
            _ => GITMOJI_UNICODE_SPEC,
        };
        parts.push(spec.to_string());
    }

    // One-liner
    if cfg.one_liner {
        parts.push("Craft a concise, single sentence, commit message that encapsulates all changes made, with an emphasis on the primary updates. If the modifications share a common theme or scope, mention it succinctly; otherwise, leave the scope out to maintain focus. The goal is to provide a clear and unified overview of the changes in one single message. Output ONLY a single-line commit message in the format: type[optional scope]: description. Do NOT include a body or footer. The entire commit message must fit on one line.".to_string());
    }

    // Locale
    if cfg.locale != "en" {
        parts.push(format!(
            "Write the commit message in the '{}' locale.",
            cfg.locale
        ));
    }

    // Universal closing instructions
    parts.push(
        "Use present tense. Be concise. Output only the raw commit message, nothing else."
            .to_string(),
    );

    parts.join("\n\n")
}

/// Strip common LLM artifacts from the raw response so only the commit message remains.
///
/// Handles:
/// - Markdown code fences (``` or ```commit / ```text / etc.)
/// - Leading label lines ("Here is your commit message:", "Commit message:", etc.)
/// - Surrounding quotation marks
pub fn clean_commit_message(raw: &str) -> String {
    let s = raw.trim();

    // Strip markdown code fences
    let s = strip_code_fence(s);

    // Strip a leading label line (everything before the first blank line or
    // the first line that looks like a conventional commit / gitmoji prefix).
    let s = strip_label_prefix(s);

    // Strip surrounding straight or curly quotes
    let s = strip_surrounding_quotes(s);

    s.trim().to_string()
}

/// Validate the exact message that would be handed to Git.
pub fn validate_commit_message(message: &str, cfg: &AppConfig) -> Result<()> {
    let message = message.trim();
    if message.is_empty() {
        anyhow::bail!("Commit message is empty");
    }
    if message.contains('\0') {
        anyhow::bail!("Commit message contains a NUL byte");
    }
    if cfg.one_liner && message.lines().count() != 1 {
        anyhow::bail!("One-liner mode requires exactly one line");
    }

    let first_line = message.lines().next().expect("non-empty message");
    let conventional = if cfg.use_gitmoji {
        strip_required_gitmoji(first_line, &cfg.gitmoji_format)?
    } else {
        first_line
    };
    static CONVENTIONAL: OnceLock<Regex> = OnceLock::new();
    let conventional_re = CONVENTIONAL
        .get_or_init(|| Regex::new(r"^[a-z][a-z0-9-]*(\([^()\r\n]+\))?!?: .*\S$").unwrap());
    if !conventional_re.is_match(conventional) {
        anyhow::bail!(
            "Commit message must start with a Conventional Commit header such as 'feat: add login'"
        );
    }
    Ok(())
}

fn strip_required_gitmoji<'a>(line: &'a str, format: &str) -> Result<&'a str> {
    let (prefix, message) = line
        .split_once(' ')
        .ok_or_else(|| anyhow::anyhow!("Gitmoji messages must start with an emoji and a space"))?;
    match format {
        "shortcode"
            if prefix.len() > 2
                && prefix.starts_with(':')
                && prefix.ends_with(':')
                && prefix[1..prefix.len() - 1]
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) =>
        {
            Ok(message)
        }
        "unicode" if !prefix.is_ascii() => Ok(message),
        "shortcode" => {
            anyhow::bail!("Gitmoji shortcode format requires a prefix such as ':sparkles:'")
        }
        _ => anyhow::bail!("Gitmoji unicode format requires an emoji prefix"),
    }
}

fn strip_code_fence(s: &str) -> &str {
    // Match opening fence with optional language tag (e.g., ```commit, ```text)
    if let Some(inner) = s.strip_prefix("```") {
        // Skip the language tag on the first line
        let after_tag = inner.trim_start_matches(|c: char| c.is_alphanumeric() || c == '-');
        // Must start with a newline after the tag
        if let Some(body) = after_tag.strip_prefix('\n') {
            if let Some(end) = body.rfind("```") {
                return body[..end].trim();
            }
        }
    }
    s
}

fn strip_label_prefix(s: &str) -> &str {
    // Common prefixes LLMs put before the actual message
    let label_patterns: &[&str] = &[
        "commit message:",
        "here is the commit message:",
        "here's the commit message:",
        "here is your commit message:",
        "here's your commit message:",
        "generated commit message:",
        "suggested commit message:",
        "the commit message:",
    ];

    let lower = s.to_lowercase();
    for pat in label_patterns {
        if let Some(rest) = lower.strip_prefix(pat) {
            // Trim blank lines / whitespace after the label
            return s[pat.len()..][rest.len() - rest.trim_start().len()..].trim_start();
        }
    }
    s
}

fn strip_surrounding_quotes(s: &str) -> &str {
    let quote_pairs: &[(char, char)] = &[('"', '"'), ('\'', '\''), ('\u{201c}', '\u{201d}')];
    for &(open, close) in quote_pairs {
        if s.starts_with(open) && s.ends_with(close) && s.len() > 1 {
            return &s[open.len_utf8()..s.len() - close.len_utf8()];
        }
    }
    s
}
