use tiktoken_rs::CoreBPE;
use tiktoken_rs::tokenizer::get_tokenizer;
use std::sync::OnceLock;

// Cached tokenizer
static TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

// Get the tokenizer, initializing it if needed
fn get_bpe() -> &'static CoreBPE {
    TOKENIZER.get_or_init(|| {
        // Use cl100k_base which is used by GPT-4 and ChatGPT
        get_tokenizer("cl100k_base").unwrap()
    })
}

// Count tokens in a string
pub fn token_count(text: &str) -> usize {
    let bpe = get_bpe();
    bpe.encode_with_special_tokens(text).len()
}