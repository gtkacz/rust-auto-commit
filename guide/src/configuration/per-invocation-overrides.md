# Per-Invocation Overrides

Any setting can be overridden for a single run with `--set KEY=VALUE` (repeatable). Overrides apply on top of all other layers and are **never persisted**:

```sh
cgen --set model=gpt-4o --set one_liner=false
cgen --set provider=ollama        # try a different provider just this once
```

Keys are the setting names from the [configuration table](config-file-and-locations.md) (case-insensitive; `-` and `_` are interchangeable, e.g. `one-liner`). Every setting is overridable except `auto_update` (a persistent global preference). Unknown keys are rejected with the list of valid keys.

To refine which files are sent to the LLM for one run, use `--diff-include`/`--diff-exclude` (see [Diff Exclusion Patterns](diff-exclusion-patterns.md)).

Use `--prompt TEXT` (or `-p`) for additive guidance that should not become
persistent configuration:

```sh
cgen --prompt "emphasize the compatibility impact on plugin authors"
cgen alter HEAD~2 --prompt "describe the migration path in the body"
```

Runtime guidance is delimited in the system prompt before cgen's mandatory
Conventional Commit, locale, output-only, and safety instructions. It can
refine content and style but cannot override those rules.

Generation flags are global and may appear before or after a subcommand.
Always quote globs so your shell does not expand them:
`--diff-include "*.xml"`.
