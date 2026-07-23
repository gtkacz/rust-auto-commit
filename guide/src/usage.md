# Usage

```
cgen                    # Generate commit message and commit
cgen --dry-run          # Generate and show message without committing
cgen --verbose          # Print final system prompt used for LLM call (diff omitted)
cgen --tag              # Create next semantic version tag after commit
cgen --set model=gpt-4o      # Override any setting for this run only (repeatable)
cgen --diff-include "*.xml"  # Force-include matching files in the LLM diff (repeatable)
cgen --diff-exclude "*.sql"  # Exclude extra files from the LLM diff this run (repeatable)
cgen --allow-large-diff      # Explicitly allow a payload over the configured byte budget
cgen --allow-sensitive       # Explicitly allow a diff flagged as sensitive
cgen --no-verify        # Forward flags to git commit
cgen alter <hash>       # Regenerate message from that commit's diff and rewrite it
cgen alter <old> <new>  # Use old..new net diff, rewrite <new> message
cgen undo               # Undo latest commit with safety prompts (soft reset)
cgen update             # Update cgen to the latest version
cgen config             # Interactive config editor (auto-detects scope)
cgen prompt             # Print the LLM system prompt without running anything
cgen history            # Browse AI-generated commits for the current repo
cgen preset             # Manage LLM presets (same UI as config menu entry)
cgen fallback           # Configure fallback order (same UI as config menu entry)
```

Any arguments passed to `cgen` (without a subcommand) are forwarded directly to `git commit`.
