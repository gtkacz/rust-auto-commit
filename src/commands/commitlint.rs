use crate::error::{Error, Result};
use crate::cli::CommitlintAction;
use crate::engine::get_engine;
use crate::commands::config::Config;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, error, debug};
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use sha2::{Sha256, Digest};

const COMMITLINT_LLM_CONFIG_PATH: &str = ".opencommit-commitlint";

#[derive(Debug, Serialize, Deserialize)]
struct CommitlintLLMConfig {
    hash: String,
    prompts: Vec<String>,
    consistency: serde_json::Map<String, Value>,
}

// Calculate a hash for a string
fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

// Check if commitlint config file exists
async fn commitlint_llm_config_exists() -> bool {
    Path::new(COMMITLINT_LLM_CONFIG_PATH).exists()
}

// Read commitlint config
async fn get_commitlint_llm_config() -> Result<CommitlintLLMConfig> {
    let content = fs::read_to_string(COMMITLINT_LLM_CONFIG_PATH)?;
    let config: CommitlintLLMConfig = serde_json::from_str(&content)?;
    Ok(config)
}

// Write commitlint config
async fn write_commitlint_llm_config(config: &CommitlintLLMConfig) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    fs::write(COMMITLINT_LLM_CONFIG_PATH, content)?;
    Ok(())
}

// Get commitlint config from project
async fn get_commitlint_pwd_config() -> Result<Value> {
    // Try to run commitlint --print-config
    let output = Command::new("npx")
        .args(&["commitlint", "--print-config"])
        .output();
        
    match output {
        Ok(output) => {
            if output.status.success() {
                let config_str = String::from_utf8_lossy(&output.stdout);
                let config: Value = serde_json::from_str(&config_str)?;
                Ok(config)
            } else {
                Err(Error::CommitlintError(
                    "Failed to get commitlint config. Make sure commitlint is installed.".to_string()
                ))
            }
        }
        Err(_) => {
            Err(Error::CommitlintError(
                "Failed to run commitlint. Make sure commitlint is installed.".to_string()
            ))
        }
    }
}

// Infer prompts from commitlint config
fn infer_prompts_from_commitlint_config(config: &Value) -> Vec<String> {
    let mut prompts = Vec::new();
    
    // Extract rules from config
    if let Some(rules) = config.get("rules").and_then(|r| r.as_object()) {
        for (rule_name, rule_config) in rules {
            if let Some(rule_array) = rule_config.as_array() {
                if rule_array.len() >= 2 {
                    // Get severity (0 = disabled, 1 = warning, 2 = error)
                    let severity = rule_array[0].as_i64().unwrap_or(0);
                    if severity == 0 {
                        continue; // Skip disabled rules
                    }
                    
                    // Get applicable (true/false)
                    let applicable = rule_array[1].as_bool().unwrap_or(true);
                    let applicable_str = if applicable { "must" } else { "must not" };
                    
                    // Get value (if any)
                    let value = if rule_array.len() > 2 { Some(&rule_array[2]) } else { None };
                    
                    // Generate prompt based on rule name
                    let prompt = match rule_name.as_str() {
                        "body-case" => {
                            if let Some(case_value) = value {
                                format!("The body should {} be in {} case.", applicable_str, case_value)
                            } else {
                                format!("The body should {} follow case rules.", applicable_str)
                            }
                        }
                        "body-empty" => {
                            format!("The body should {} be empty.", applicable_str)
                        }
                        "body-full-stop" => {
                            if let Some(stop_value) = value {
                                format!("The body should {} end with '{}'.", applicable_str, stop_value)
                            } else {
                                format!("The body should {} end with a full stop.", applicable_str)
                            }
                        }
                        "body-leading-blank" => {
                            format!("There should {} be a blank line at the beginning of the body.", applicable_str)
                        }
                        "body-max-length" => {
                            if let Some(length) = value {
                                format!("The body should {} have {} characters or less.", applicable_str, length)
                            } else {
                                format!("The body should {} have a maximum length.", applicable_str)
                            }
                        }
                        "type-enum" => {
                            if let Some(types) = value.and_then(|v| v.as_array()) {
                                let types_str = types.iter()
                                    .filter_map(|t| t.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!("The type should {} be one of the following: {}.", applicable_str, types_str)
                            } else {
                                format!("The type should {} be from the allowed list.", applicable_str)
                            }
                        }
                        // Add more rule handlers as needed
                        _ => {
                            format!("Rule '{}' should {} be followed.", rule_name, applicable_str)
                        }
                    };
                    
                    prompts.push(prompt);
                }
            }
        }
    }
    
    prompts
}

// Configure commitlint integration
async fn configure_commitlint_integration(force: bool) -> Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner} {msg}")
            .unwrap(),
    );
    spinner.set_message("Loading @commitlint configuration");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let file_exists = commitlint_llm_config_exists().await;
    
    let commitlint_config = get_commitlint_pwd_config().await?;
    let config_json = serde_json::to_string(&commitlint_config)?;
    let hash = compute_hash(&config_json);
    
    spinner.set_message(format!("Read @commitlint configuration (hash: {})", hash));
    
    if file_exists && !force {
        // Check if we need to update the prompts
        let existing_config = get_commitlint_llm_config().await?;
        if existing_config.hash == hash {
            spinner.finish_with_message(
                "Hashes are the same, no need to update the config. Run \"force\" command to bypass."
            );
            return Ok(());
        }
    }
    
    spinner.set_message("Generating consistency with given @commitlint rules");
    
    let prompts = infer_prompts_from_commitlint_config(&commitlint_config);
    
    // Load config
    let config = Config::load()?;
    
    // Generate consistency with OpenAI
    let engine = crate::engine::get_engine(&config)?;
    
    // Create prompts for consistency
    let messages = crate::prompts::get_commitlint_consistency_prompt(&prompts).await?;
    
    // Example diff for consistency
    let diff = r#"diff --git a/src/server.ts b/src/server.ts
index ad4db42..f3b18a9 100644
--- a/src/server.ts
+++ b/src/server.ts
@@ -10,7 +10,7 @@
import {
    initWinstonLogger();
    
    const app = express();
    -const port = 7799;
    +const PORT = 7799;
    
    app.use(express.json());
    
    @@ -34,6 +34,6 @@
    app.use((_, res, next) => {
        // ROUTES
        app.use(PROTECTED_ROUTER_URL, protectedRouter);
        
        -app.listen(port, () => {
            -  console.log(\`Server listening on port \${port}\`);
            +app.listen(process.env.PORT || PORT, () => {
                +  console.log(\`Server listening on port \${PORT}\`);
            });"#;
    
    let consistency = engine.generate_commit_message(messages, diff).await?;
    
    // Extract JSON from response if needed
    if let Some(json_start) = consistency.find('{') {
        if let Some(json_end) = consistency.rfind('}') {
            consistency = consistency[json_start..=json_end].to_string();
        }
    }
    
    // Parse JSON
    let consistency_json: Value = serde_json::from_str(&consistency)?;
    
    // Get local language
    let local_language = consistency_json.get("localLanguage")
        .and_then(|l| l.as_str())
        .unwrap_or("english")
        .to_string();
    
    // Create config
    let mut consistency_map = serde_json::Map::new();
    consistency_map.insert(local_language, consistency_json);
    
    let llm_config = CommitlintLLMConfig {
        hash,
        prompts,
        consistency: consistency_map,
    };
    
    // Write config
    write_commitlint_llm_config(&llm_config).await?;
    
    spinner.finish_with_message(
        format!("Done - please review contents of {}", COMMITLINT_LLM_CONFIG_PATH)
    );
    
    Ok(())
}

// Handler for commitlint commands
pub async fn handle_commitlint_command(action: CommitlintAction) -> Result<()> {
    println!("{}", "OpenCommit Commitlint".bright_blue());
    
    match action {
        CommitlintAction::Get => {
            if !commitlint_llm_config_exists().await {
                return Err(Error::CommitlintError(
                    format!("Config file {} does not exist. Run `oco commitlint force` to create it.", COMMITLINT_LLM_CONFIG_PATH)
                ));
            }
            
            let config = get_commitlint_llm_config().await?;
            println!("{}", serde_json::to_string_pretty(&config)?);
            
            Ok(())
        }
        CommitlintAction::Force => {
            configure_commitlint_integration(true).await
        }
    }
}