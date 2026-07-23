# Per-Invocation Overrides

Any setting can be overridden for a single run with `--set KEY=VALUE` (repeatable). Overrides apply on top of all other layers and are **never persisted**:

```sh
cgen --set model=gpt-4o --set one_liner=false
cgen --set provider=ollama        # try a different provider just this once
```

Keys are the setting names from the [configuration table](config-file-and-locations.md) (case-insensitive; `-` and `_` are interchangeable, e.g. `one-liner`). Every setting is overridable except `auto_update` (a persistent global preference). Unknown keys are rejected with the list of valid keys.

To refine which files are sent to the LLM for one run, use `--diff-include`/`--diff-exclude` (see [Diff Exclusion Patterns](diff-exclusion-patterns.md)).

Generation flags are global and may appear before or after a subcommand.
Always quote globs so your shell does not expand them:
`--diff-include "*.xml"`.
