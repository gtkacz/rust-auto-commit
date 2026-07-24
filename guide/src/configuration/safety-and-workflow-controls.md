# Safety & Workflow Controls

cgen is deliberately conservative: nothing is sent to a provider or written to
history without either a default guard or an explicit override.

## Pre-flight checks

Before generating, cgen prints the staged file count and names; files excluded
from the LLM payload are marked `(not sent to LLM)`. If staged files exceed
`ACR_WARN_STAGED_FILES_THRESHOLD`, or the files actually sent to the LLM exceed
`ACR_WARN_LLM_FILES_THRESHOLD`, cgen asks one merged confirmation (including
the payload size in KB) before continuing. Each check can be disabled with its
`_ENABLED` flag.

## Sensitive data guard

Sensitive filenames (`ACR_SENSITIVE_FILE_GLOBS`, which defaults to `.env`
files, private keys, and credential-like paths) and high-confidence credential
patterns in the diff content are blocked before any provider request. Use
`--allow-sensitive` only after reviewing the exact staged diff.

## Large diff guard

Filtered diffs above `ACR_MAX_DIFF_BYTES` (default 200 KB) are blocked before
any provider request; `--allow-large-diff` is an explicit one-run override.

## Inspecting without acting

- `cgen --dry-run` generates and prints the final commit message but does not create a commit.
- `cgen alter --dry-run` generates and prints the rewritten message but does not rewrite history.
- `cgen --verbose` prints the final system prompt sent to the LLM and never prints the diff payload.
- `cgen prompt` prints the full LLM system prompt (based on current config) without running any LLM call or git operations.

## Pushing

After a real commit, push behavior follows `ACR_POST_COMMIT_PUSH`:

- `never`: never push
- `ask`: prompt whether to push (default)
- `always`: push automatically

## Semantic version tags (`--tag`)

`cgen --tag` creates a semantic version tag after a successful commit:

- no existing tag → `0.1.0`
- latest semver tag `x.y.z` → `x.(y+1).0`
- latest tag not in semantic versioning → error

If `ACR_CONFIRM_NEW_VERSION=1` (default), cgen asks before creating the
computed tag; if `0`, it creates it directly. The tag is pushed explicitly
after a successful branch push; partial failures report that the tag remains
local.

## Rewriting history (`cgen alter`)

- `cgen alter <hash>` regenerates that commit's message from its own diff; `cgen alter <old> <new>` uses the `old..new` net diff as LLM input and rewrites only the `<new>` commit message.
- If the target commit is already pushed, cgen requires explicit confirmation before rewriting.
- For rewritten pushed history, cgen offers a separate, default-No `git push --force-with-lease` action; it never performs an unguarded force push.

## Undoing (`cgen undo`)

`cgen undo` only undoes the latest commit, keeps its changes staged (including
a repository's root commit), never pushes, and warns before undoing pushed
commits.
