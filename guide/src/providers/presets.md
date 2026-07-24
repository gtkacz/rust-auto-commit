# Presets

A preset is a named snapshot of the five provider-related settings: provider,
model, API key, API URL, and API headers. Presets make it cheap to switch
between setups (say, a fast local Ollama model and a cloud model for tricky
diffs) and they feed the [fallback mechanism](fallback-order.md), which walks
your presets when the primary provider fails.

Manage presets from the `cgen config` interactive menu, or directly with
`cgen preset`:

- **Save current as preset**: saves the current provider/model/key/url/headers as a named preset (offered only when no identical preset exists)
- **Load a preset**: applies a saved preset to the current config session; if you then modify a loaded preset's fields, cgen offers to update the preset on save
- **Manage presets**: create, rename, duplicate, delete, export, and import presets
- **Export/Import**: export presets as TOML (optionally redacting API keys) for sharing or backup

Presets are stored in `{config_dir}/cgen/presets.toml` alongside the global
config. Deduplication uses `(provider, model, api_key, api_url)` as the key.
