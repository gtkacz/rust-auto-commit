# Built-in Providers

Built-in providers: **Groq** (default), **OpenAI**, **Anthropic**, **Gemini**, **Grok**, **DeepSeek**, **OpenRouter**, **Mistral**, **Together**, **Fireworks**, **Perplexity**, **LM Studio**, **Ollama**.

| Provider | Default Model |
|----------|---------------|
| groq | llama-3.3-70b-versatile |
| openai | gpt-4o-mini |
| anthropic | claude-sonnet-4-20250514 |
| gemini | gemini-2.0-flash |
| grok | grok-3 |
| deepseek | deepseek-chat |
| openrouter | openai/gpt-4o-mini |
| mistral | mistral-small-latest |
| together | meta-llama/Llama-3.3-70B-Instruct-Turbo |
| fireworks | accounts/fireworks/models/llama-v3p3-70b-instruct |
| perplexity | sonar |
| lm_studio | qwen/qwen3.5-35b-a3b |
| ollama | llama3 |

For custom providers, set `ACR_PROVIDER` to any name and provide `ACR_API_URL`. Custom providers default to OpenAI-compatible request format.

```sh
export ACR_PROVIDER=vllm
export ACR_API_URL=http://localhost:8000/v1/chat/completions
export ACR_MODEL=meta-llama/Llama-3-8B
```
