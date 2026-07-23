use crate::config::AppConfig;
use anyhow::{Context, Result};
use regex_lite::Regex;
use std::sync::OnceLock;

/// Interpolate `$VARIABLE_NAME` patterns in a string using environment variables.
///
/// Config-backed `ACR_*` values are resolved directly and the process
/// environment is never mutated. Missing variables are errors instead of being
/// silently erased from URLs or headers.
pub fn interpolate(template: &str, cfg: &AppConfig) -> Result<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap());
    let mut result = String::with_capacity(template.len());
    let mut end = 0;

    for captures in re.captures_iter(template) {
        let full = captures.get(0).expect("full regex match");
        let name = captures.get(1).expect("variable capture").as_str();
        result.push_str(&template[end..full.start()]);
        let value = match name {
            "ACR_PROVIDER" => cfg.provider.clone(),
            "ACR_MODEL" => cfg.model.clone(),
            "ACR_API_KEY" => cfg.api_key.clone(),
            "ACR_LOCALE" => cfg.locale.clone(),
            _ => std::env::var(name)
                .with_context(|| format!("Environment variable '{name}' is not set"))?,
        };
        result.push_str(&value);
        end = full.end();
    }
    result.push_str(&template[end..]);
    Ok(result)
}
