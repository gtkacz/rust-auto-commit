use anyhow::{Context, Result};

use crate::config::AppConfig;
use crate::prompt;
use crate::provider::{self, OutputMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCandidate {
    pub message: String,
    pub fallback_preset: Option<String>,
}

/// Generate independently validated candidates in sequence. Earlier results
/// are sent as alternative context, but each candidate still receives the full
/// provider fallback and corrective-validation workflow.
pub fn generate_candidates(
    cfg: &AppConfig,
    diff: &str,
    count: usize,
    guidance: Option<&str>,
    output_mode: OutputMode,
) -> Result<Vec<GeneratedCandidate>> {
    if count == 0 {
        anyhow::bail!("candidate count must be greater than zero");
    }

    let system_prompt = prompt::build_system_prompt_with_guidance(cfg, guidance);
    let mut candidates = Vec::with_capacity(count);
    let mut previous = Vec::with_capacity(count);
    for _ in 0..count {
        let (message, fallback_preset) = provider::generate_validated_message_with_context(
            cfg,
            &system_prompt,
            diff,
            &previous,
            output_mode,
        )?;
        previous.push(message.clone());
        candidates.push(GeneratedCandidate {
            message,
            fallback_preset,
        });
    }
    Ok(candidates)
}

pub fn apply_template(cfg: &AppConfig, message: &str) -> Result<String> {
    let final_message = cfg
        .commit_template
        .replace("$msg", message.trim())
        .trim()
        .to_string();
    prompt::validate_final_message(&final_message)
        .context("Commit template produced an invalid commit message")?;
    Ok(final_message)
}
