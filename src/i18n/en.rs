use crate::i18n::TranslationData;

pub fn get_translation() -> TranslationData {
    TranslationData {
        local_language: "english".to_string(),
        commit_fix: "fix(server.ts): change port variable case from lowercase port to uppercase PORT to improve semantics".to_string(),
        commit_feat: "feat(server.ts): add support for process.env.PORT environment variable to be able to run app on a configurable port".to_string(),
        commit_description: "The port variable is now named PORT, which improves consistency with the naming conventions as PORT is a constant. Support for an environment variable allows the application to be more flexible as it can now run on any available port specified via the process.env.PORT environment variable.".to_string(),
    }
}