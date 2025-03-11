use crate::i18n::TranslationData;

pub fn get_translation() -> TranslationData {
    TranslationData {
        local_language: "portuguese".to_string(),
        commit_fix: "fix(server.ts): alterar a caixa da variável de minúscula port para maiúscula PORT para melhorar a semântica".to_string(),
        commit_feat: "feat(server.ts): adicionar suporte para a variável de ambiente process.env.PORT para poder executar o aplicativo em uma porta configurável".to_string(),
        commit_description: "A variável de porta agora é chamada PORT, o que melhora a consistência com as convenções de nomenclatura, pois PORT é uma constante. O suporte para uma variável de ambiente permite que o aplicativo seja mais flexível, pois agora ele pode ser executado em qualquer porta disponível especificada por meio da variável de ambiente process.env.PORT.".to_string(),
    }
}