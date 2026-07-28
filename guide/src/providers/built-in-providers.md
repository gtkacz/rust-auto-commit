# Built-in Providers

Built-in providers work with just `ACR_PROVIDER` and (for cloud providers)
`ACR_API_KEY` — the endpoint URL, request format, and headers are resolved
automatically. Selecting a provider in `cgen config` also sets its default
model, which you can override with `ACR_MODEL`.

Run `cgen model` (or edit **Model** inside `cgen config`) to fetch the
provider's current model catalog, search it, and persist a selection. Discovery
uses short timeouts and bounded responses, authenticates with the configured
credentials, and filters non-generation models when the provider exposes that
metadata. OpenAI, Anthropic, Gemini, Groq, Grok, DeepSeek, OpenRouter, Mistral,
Together, Fireworks, LM Studio, and Ollama support live discovery. Perplexity
uses the manual fallback because its current list endpoint is for a different
API surface.

| Provider | Default Model | API key |
|----------|---------------|---------|
| `groq` (default) | llama-3.3-70b-versatile | required |
| `openai` | gpt-4o-mini | required |
| `anthropic` | claude-sonnet-4-20250514 | required |
| `gemini` | gemini-2.0-flash | required |
| `grok` | grok-3 | required |
| `deepseek` | deepseek-chat | required |
| `openrouter` | openai/gpt-4o-mini | required |
| `mistral` | mistral-small-latest | required |
| `together` | meta-llama/Llama-3.3-70B-Instruct-Turbo | required |
| `fireworks` | accounts/fireworks/models/llama-v3p3-70b-instruct | required |
| `perplexity` | sonar | required |
| `lm_studio` | qwen/qwen3.5-35b-a3b | not needed (local) |
| `ollama` | llama3 | not needed (local) |

`lm_studio` and `ollama` talk to a locally running server, so commit generation
stays entirely on your machine.

## Custom providers

Set `ACR_PROVIDER` to any other name and provide `ACR_API_URL`. Custom
providers default to the OpenAI-compatible request format, and both the URL and
`ACR_API_HEADERS` support [variable interpolation](../configuration/variable-interpolation.md):

```sh
export ACR_PROVIDER=vllm
export ACR_API_URL=http://localhost:8000/v1/chat/completions
export ACR_MODEL=meta-llama/Llama-3-8B
```

For custom providers, `cgen model` derives `/models` only when the configured
URL ends in the recognizable `/chat/completions` path. Other endpoint shapes
fall back to current/default/manual selection rather than guessing a URL.

To make a provider available to everyone as a built-in, see
[Adding a New Default Provider](../contributing.md#adding-a-new-default-provider)
— it's usually a 5-line change.
