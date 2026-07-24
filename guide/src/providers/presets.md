# Presets

Presets let you save and reuse LLM provider configurations. Manage them from the `cgen config` interactive menu:

- **Save current as preset**: saves the current provider/model/key/url/headers as a named preset
- **Load a preset**: applies a saved preset to the current config session
- **Manage presets**: create, rename, duplicate, delete, export, and import presets
- **Export/Import**: export presets as TOML (optionally redacting API keys) for sharing or backup

Presets are stored in `{config_dir}/cgen/presets.toml` alongside the global config. Deduplication uses `(provider, model, api_key, api_url)` as the key.
