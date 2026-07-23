# Diff Exclusion Patterns

`ACR_DIFF_EXCLUDE_GLOBS` filters files from the diff sent to the LLM while still committing them. This reduces noise and token usage for binary, generated, or data files. Default patterns:

```
*.json, *.xml, *.csv, *.pdf, *.lock, *.svg, *.png, *.jpg, *.jpeg, *.gif, *.ico, *.woff, *.woff2, *.ttf, *.eot, *.min.js, *.min.css
```

Override with a comma-separated list:

```sh
export ACR_DIFF_EXCLUDE_GLOBS="*.lock,*.svg,package-lock.json"
```

For a single run, adjust the effective filter without touching your config:

```sh
cgen --diff-include "*.xml"   # send .xml files to the LLM even though they're excluded
cgen --diff-exclude "*.sql"   # additionally drop .sql files from the LLM diff
```

`--diff-include` wins over any exclude pattern (allow-over-deny). Patterns with
a slash match repository-relative paths; basename patterns such as `*.lock`
match at any depth. Invalid patterns and filters that remove every changed file
are errors. The same filters apply to normal generation and `cgen alter`.

Note: `ACR_AUTO_UPDATE` is a global-only setting and is not written to local `.env` files.
