use anyhow::{Context, Result};
use colored::Colorize;
use inquire::{Select, Text};
use serde_json::Value;
use std::collections::BTreeSet;
use std::time::Duration;

use crate::config::AppConfig;
use crate::interpolation::interpolate;
use crate::provider::{self, ModelListParser};

const MODEL_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_MODEL_RESPONSE_BYTES: usize = 1_048_576;

pub fn discover_models(cfg: &AppConfig) -> Result<Vec<String>> {
    let spec = provider::model_list_spec(cfg).context(
        "This provider does not expose a compatible model-list endpoint; enter a model manually",
    )?;
    let url = interpolate(&spec.url, cfg)?;
    let raw_headers = interpolate(&spec.headers, cfg)?;
    let headers = provider::parse_headers(&raw_headers)?;

    let agent = ureq::Agent::new_with_defaults();
    let mut request = agent.get(&url);
    for (name, value) in headers {
        request = request.header(&name, &value);
    }
    let response = request
        .config()
        .timeout_global(Some(MODEL_REQUEST_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .call()
        .map_err(|error| {
            anyhow::anyhow!(
                "Model discovery request failed: {}",
                provider::redact(&error.to_string(), &cfg.api_key)
            )
        })?;
    let status = response.status().as_u16();
    let body =
        provider::read_bounded(response.into_body().into_reader(), MAX_MODEL_RESPONSE_BYTES)?;
    if status >= 400 {
        anyhow::bail!(
            "Model endpoint returned HTTP {status}: {}",
            bounded_diagnostic(&body, &cfg.api_key)
        );
    }
    let json: Value =
        serde_json::from_str(&body).context("Model endpoint returned invalid JSON")?;
    parse_model_list(&json, spec.parser)
}

pub fn parse_model_list(value: &Value, parser: ModelListParser) -> Result<Vec<String>> {
    let mut models = BTreeSet::new();
    match parser {
        ModelListParser::Data => {
            let entries = value
                .get("data")
                .and_then(Value::as_array)
                .or_else(|| value.get("models").and_then(Value::as_array))
                .or_else(|| value.as_array())
                .context("Model response did not contain a model array")?;
            for entry in entries {
                if !generation_capable(entry) {
                    continue;
                }
                if let Some(id) = model_identifier(entry) {
                    models.insert(id.to_string());
                }
            }
        }
        ModelListParser::Gemini => {
            let entries = value
                .get("models")
                .and_then(Value::as_array)
                .context("Gemini response did not contain a models array")?;
            for entry in entries {
                let supports_generation = entry
                    .get("supportedGenerationMethods")
                    .and_then(Value::as_array)
                    .is_none_or(|methods| {
                        methods
                            .iter()
                            .filter_map(Value::as_str)
                            .any(|method| method == "generateContent")
                    });
                if supports_generation {
                    if let Some(name) = entry.get("name").and_then(Value::as_str) {
                        models.insert(name.strip_prefix("models/").unwrap_or(name).to_string());
                    }
                }
            }
        }
        ModelListParser::Models => {
            let entries = value
                .get("models")
                .and_then(Value::as_array)
                .context("Model response did not contain a models array")?;
            for entry in entries {
                if generation_capable(entry) {
                    if let Some(id) = model_identifier(entry) {
                        models.insert(id.to_string());
                    }
                }
            }
        }
        ModelListParser::Ollama => {
            let entries = value
                .get("models")
                .and_then(Value::as_array)
                .context("Ollama response did not contain a models array")?;
            for entry in entries {
                if let Some(name) = entry.get("name").and_then(Value::as_str) {
                    models.insert(name.to_string());
                }
            }
        }
    }
    if models.is_empty() {
        anyhow::bail!("Model endpoint returned no generation-capable models");
    }
    Ok(models.into_iter().collect())
}

pub fn select_model(cfg: &AppConfig) -> Result<Option<String>> {
    match discover_models(cfg) {
        Ok(models) => select_from_models(cfg, models),
        Err(error) => {
            eprintln!(
                "{} Model discovery failed: {}",
                "warning:".yellow().bold(),
                provider::redact(&format!("{error:#}"), &cfg.api_key)
            );
            select_manual_fallback(cfg)
        }
    }
}

pub fn run_model_command() -> Result<()> {
    let in_repo = crate::git::find_repo_root().is_ok();
    let global = if in_repo {
        match Select::new(
            "Save model locally or globally?",
            vec!["Local (.env in repo)", "Global (TOML config)"],
        )
        .prompt()
        {
            Ok(choice) => choice.starts_with("Global"),
            Err(_) => return Ok(()),
        }
    } else {
        true
    };

    if global {
        let cfg = AppConfig::load_global_for_edit()?;
        if let Some(model) = select_model(&cfg)? {
            crate::config::save_global_model(&model)?;
            println!("{} {}", "Model saved:".green().bold(), model);
        }
    } else {
        let mut state = AppConfig::load_local_for_edit()?;
        if let Some(model) = select_model(&state.config)? {
            state.config.model = model.clone();
            state.explicit_fields.insert("MODEL".into());
            state.config.save_local_overrides(&state.explicit_fields)?;
            println!("{} {}", "Model saved:".green().bold(), model);
        }
    }
    Ok(())
}

fn select_from_models(cfg: &AppConfig, models: Vec<String>) -> Result<Option<String>> {
    let ordered = prioritize_models(cfg, models);
    let mut query = String::new();
    loop {
        let filtered = ordered
            .iter()
            .filter(|model| {
                query.is_empty()
                    || model
                        .to_ascii_lowercase()
                        .contains(&query.to_ascii_lowercase())
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut choices = filtered;
        choices.push("Search models...".into());
        choices.push("Enter a custom model...".into());
        let choice = match Select::new("Model:", choices).with_page_size(20).prompt() {
            Ok(choice) => choice,
            Err(_) => return Ok(None),
        };
        match choice.as_str() {
            "Search models..." => {
                query = match Text::new("Search:").with_default(&query).prompt() {
                    Ok(query) => query.trim().to_string(),
                    Err(_) => continue,
                };
            }
            "Enter a custom model..." => return prompt_custom_model(),
            _ => return Ok(Some(choice)),
        }
    }
}

fn select_manual_fallback(cfg: &AppConfig) -> Result<Option<String>> {
    let default = provider::default_model_for(&cfg.provider);
    let mut choices = Vec::new();
    if !cfg.model.trim().is_empty() {
        choices.push(format!("Keep current: {}", cfg.model));
    }
    if !default.is_empty() && default != cfg.model {
        choices.push(format!("Use default: {default}"));
    }
    choices.push("Enter a custom model...".into());
    match Select::new("Model:", choices).prompt() {
        Ok(choice) if choice == "Enter a custom model..." => prompt_custom_model(),
        Ok(choice) => Ok(choice.split_once(": ").map(|(_, model)| model.to_string())),
        Err(_) => Ok(None),
    }
}

fn prompt_custom_model() -> Result<Option<String>> {
    match Text::new("Model identifier:").prompt() {
        Ok(model) if !model.trim().is_empty() => Ok(Some(model.trim().to_string())),
        Ok(_) => {
            eprintln!("{} Model cannot be empty", "error:".red().bold());
            Ok(None)
        }
        Err(_) => Ok(None),
    }
}

fn prioritize_models(cfg: &AppConfig, models: Vec<String>) -> Vec<String> {
    let default = provider::default_model_for(&cfg.provider);
    let mut ordered = Vec::with_capacity(models.len() + 2);
    for preferred in [cfg.model.as_str(), default] {
        if !preferred.is_empty() && !ordered.iter().any(|model| model == preferred) {
            ordered.push(preferred.to_string());
        }
    }
    for model in models {
        if !ordered.iter().any(|existing| existing == &model) {
            ordered.push(model);
        }
    }
    ordered
}

fn model_identifier(value: &Value) -> Option<&str> {
    value
        .get("id")
        .or_else(|| value.get("key"))
        .or_else(|| value.get("name"))
        .or_else(|| value.get("model"))
        .and_then(Value::as_str)
}

fn generation_capable(value: &Value) -> bool {
    if let Some(kind) = value.get("type").and_then(Value::as_str) {
        let kind = kind.to_ascii_lowercase();
        if kind.contains("embed") || kind.contains("rerank") {
            return false;
        }
    }
    if let Some(capabilities) = value.get("capabilities").and_then(Value::as_object) {
        if capabilities
            .get("completion_chat")
            .or_else(|| capabilities.get("chat_completion"))
            .and_then(Value::as_bool)
            == Some(false)
        {
            return false;
        }
    }
    true
}

fn bounded_diagnostic(body: &str, api_key: &str) -> String {
    provider::redact(&body.chars().take(2_048).collect::<String>(), api_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_and_sorts_openai_family() {
        let value = json!({"data": [
            {"id": "z-chat"},
            {"id": "a-chat"},
            {"id": "a-chat"},
            {"id": "embedding", "type": "embedding"}
        ]});
        assert_eq!(
            parse_model_list(&value, ModelListParser::Data).unwrap(),
            vec!["a-chat", "z-chat"]
        );
    }

    #[test]
    fn filters_gemini_generation_methods() {
        let value = json!({"models": [
            {"name": "models/gemini-chat", "supportedGenerationMethods": ["generateContent"]},
            {"name": "models/embed", "supportedGenerationMethods": ["embedContent"]}
        ]});
        assert_eq!(
            parse_model_list(&value, ModelListParser::Gemini).unwrap(),
            vec!["gemini-chat"]
        );
    }

    #[test]
    fn parses_ollama_names() {
        let value = json!({"models": [{"name": "llama3:latest"}, {"name": "qwen:7b"}]});
        assert_eq!(
            parse_model_list(&value, ModelListParser::Ollama).unwrap(),
            vec!["llama3:latest", "qwen:7b"]
        );
    }

    #[test]
    fn parses_models_family_identifiers_and_filters_capabilities() {
        let value = json!({"models": [
            {"key": "key-chat"},
            {"name": "name-chat"},
            {"model": "model-chat"},
            {"id": "disabled", "capabilities": {"chat_completion": false}},
            {"id": "reranker", "type": "rerank"}
        ]});

        assert_eq!(
            parse_model_list(&value, ModelListParser::Models).unwrap(),
            vec!["key-chat", "model-chat", "name-chat"]
        );
    }

    #[test]
    fn model_list_parsers_reject_missing_or_empty_arrays() {
        for parser in [
            ModelListParser::Data,
            ModelListParser::Gemini,
            ModelListParser::Models,
            ModelListParser::Ollama,
        ] {
            assert!(parse_model_list(&json!({}), parser).is_err());
        }
        assert!(parse_model_list(&json!([]), ModelListParser::Data).is_err());
    }

    #[test]
    fn prioritizes_current_and_default_models_without_duplicates() {
        let cfg = AppConfig {
            provider: "openai".into(),
            model: "current-model".into(),
            ..AppConfig::default()
        };
        let default = provider::default_model_for(&cfg.provider).to_string();

        assert_eq!(
            prioritize_models(
                &cfg,
                vec![
                    "other-model".into(),
                    default.clone(),
                    "current-model".into()
                ]
            ),
            vec![
                "current-model".to_string(),
                default,
                "other-model".to_string()
            ]
        );
    }

    #[test]
    fn bounded_diagnostics_truncate_and_redact_secrets() {
        let body = format!("token=secret-key {}", "x".repeat(3_000));
        let diagnostic = bounded_diagnostic(&body, "secret-key");

        assert!(!diagnostic.contains("secret-key"));
        assert!(diagnostic.len() <= 2_048);
    }
}
