use std::collections::HashMap;
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use crate::error::{Error, Result};

mod en;
mod pt_br;

// Translation data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationData {
    pub local_language: String,
    pub commit_fix: String,
    pub commit_feat: String,
    pub commit_description: String,
}

// Map of language code to aliases
static LANGUAGE_ALIASES: Lazy<HashMap<&'static str, Vec<&'static str>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("en", vec!["en", "english", "English"]);
    map.insert("pt_br", vec!["pt_br", "pt-br", "portuguese", "Portuguese", "Brazilian Portuguese", "Português", "Português Brasileiro"]);
    map
});

// Map of language code to translation data
static TRANSLATIONS: Lazy<HashMap<&'static str, TranslationData>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("en", en::get_translation());
    map.insert("pt_br", pt_br::get_translation());
    map
});

// Get language code from alias
pub fn get_language_code(alias: &str) -> Result<&'static str> {
    for (code, aliases) in LANGUAGE_ALIASES.iter() {
        if aliases.iter().any(|a| a.eq_ignore_ascii_case(alias)) {
            return Ok(code);
        }
    }
    
    Err(Error::InvalidConfiguration(format!("Unsupported language: {}", alias)))
}

// Get translation data for a language
pub fn get_translation(language: &str) -> Result<TranslationData> {
    let code = get_language_code(language)?;
    
    TRANSLATIONS.get(code)
        .cloned()
        .ok_or_else(|| Error::InvalidConfiguration(format!("Translation not found for language: {}", language)))
}

// Check if a language is supported
pub fn is_language_supported(language: &str) -> bool {
    get_language_code(language).is_ok()
}

// Get all supported languages
pub fn get_supported_languages() -> Vec<&'static str> {
    LANGUAGE_ALIASES.keys().cloned().collect()
}