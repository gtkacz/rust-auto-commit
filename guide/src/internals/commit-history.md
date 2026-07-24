# Commit History

When `ACR_TRACK_GENERATED_COMMITS=1` (default), cgen records each AI-generated commit hash and message preview in a per-repository cache.

- `cgen history` inside a git repo shows that repo's tracked commits
- `cgen history` outside a git repo lists all tracked repos, then shows commits for the selected one
- Selecting a commit runs `git show` on it
- Cache is stored in `{config_dir}/cgen/cache/`, is concurrency-safe, deduplicates rewritten hashes, and retains the latest 200 entries per repository
