# Prompt design

Rationale for the base prompts assembled in `src/prompt.rs` (`build_system_prompt`,
`build_user_prompt`) and `src/config.rs` (`DEFAULT_SYSTEM_PROMPT`). Grounded in
current vendor guidance and the empirical literature on LLM commit-message
generation (reviewed July 2026). Revisit when models or the cited guidance change
materially.

## Principles applied

### 1. Constraint-focused instructions beat minimal prompts

An empirical study of commit-message generation via in-context learning
(arXiv:2502.18904) compared four prompts and found the best performer was the one
with explicit output constraints ("do not write explanations, reply with only the
commit message"), with prompt choice mattering most in zero-shot settings — the
regime this CLI runs in against small local models. The system prompt therefore
states hard constraints (raw message only, exact header shape, one-line mode
precedence) rather than relying on the model to infer them.

### 2. Few-shot examples steer format more reliably than rules

Anthropic's prompting guidance calls examples "one of the most reliable ways to
steer output format"; the ICL study above measured double-digit metric gains from
demonstrations; Google's prompt-engineering whitepaper recommends always including
them. `build_system_prompt` injects a small `<examples>` block chosen to match the
active output mode:

- plain + one-liner → single-line headers only (`CONVENTIONAL_EXAMPLES_ONE_LINER`)
- plain + full → one bare header plus one header-with-body (`CONVENTIONAL_EXAMPLES_FULL`)
- gitmoji → the emoji header examples embedded in the gitmoji spec

Examples are selected per mode because a mismatched demonstration (e.g. a body
example in one-liner mode, or an emoji-less example in gitmoji mode) is a stronger
mis-steer than a missing rule — models copy patterns before they follow prose.

### 3. Delimit variable data; restate the task after it

OpenAI's guidance separates instructions from data with delimiters; Anthropic
recommends wrapping variable input in XML-style tags and, for long inputs, placing
the query after the data (measured up to ~30% quality improvement); the
"lost in the middle" result (Liu et al., 2023) puts critical instructions first and
last, never mid-prompt. `build_user_prompt` wraps the diff in a `<diff>` block and
restates the task plus the raw-output constraint after it, so the last thing the
model reads before generating is the output contract. The system prompt ends with
the same closing constraints for the short-diff case.

### 4. The diff is data, not instructions

Diff content is untrusted: a staged README edit can contain imperative sentences.
The default system prompt says to treat `<diff>` content strictly as data to
describe — the standard mitigation for indirect prompt injection in
data-processing prompts.

### 5. Anti-fabrication rules target known CMG failure modes

The empirical study arXiv:2404.14824 identifies the recurring failure modes of
LLM-generated commit messages: fabricated or missing "why" (the diff rarely
contains motivation), vague descriptions ("update code"), and omitted essentials.
The prompts respond directly: "why" is requested only *when the diff makes it
evident*, footers may only state facts the diff supports (the previous prompt's
`Reviewed-by: Name` example actively invited hallucinated trailers), and the
closing instruction demands concrete component names over generic phrases.

### 6. No contradictory instructions; explicit precedence

OpenAI's GPT-5 prompting guide stresses that contradictory prompt instructions
measurably degrade strong instruction-followers. The one-liner rule used to
coexist silently with "Body: OPTIONAL"; it now states "even where the rules above
allow them", and the locale rule pins header tokens (`type`, scope,
`BREAKING CHANGE`, gitmoji shortcodes) to English so translation cannot break the
`validate_commit_message` regex, which only accepts ASCII lowercase types.

### 7. Motivate constraints

Anthropic's guidance: explaining *why* a constraint exists improves adherence
("Claude is smart enough to generalize from the explanation"). The raw-output rule
carries its reason — "because your reply is passed verbatim to `git commit`".

### 8. Imperative mood, not "present tense"

"Use present tense" permits "adds login" ("adds" is present tense). Git and
Conventional Commits convention is the imperative mood, so the prompt spells out
the contrast: "add", not "added" or "adds".

### 9. Structure and economy

Anthropic's context-engineering guidance recommends delimited sections and the
minimal set of information that fully specifies behavior; practitioner guidance
converges on short, sectioned prompts (attention cost grows with length, and long
prompts are harder to debug). The assembled system prompt stays a few hundred
tokens — deliberate headroom for the small-context local models this tool
supports via Ollama/LM Studio. Static content (system prompt) precedes variable
content (diff), which is also the cache-friendly ordering for providers with
prompt caching.

### 10. Self-correction on validation failure

Feedback-driven retry (Self-Refine / Reflexion-style) is the standard recovery
for structured-output misses: instead of failing hard when
`validate_commit_message` rejects the model's output,
`provider::generate_validated_message` sends one corrective turn built by
`build_correction_prompt` — the diff again, the rejected attempt in
`<previous_attempt>` tags, the validator error in `<error>` tags, and the task
restated last. One retry captures most format failures (the common case for
small local models); a second rarely converges and doubles cost, so failure
after the corrective turn is surfaced as an error.

### 11. Default-prompt upgrades reach existing configs

Config files persist `llm_system_prompt` verbatim, which would pin users who
never customized it to whatever default text their config was first written
with. `base_prompt_is_default` treats a blank value — the new persisted
default — or any retired shipped default (whitespace-normalized comparison
against `LEGACY_SYSTEM_PROMPTS`) as "not customized" and substitutes the current
`DEFAULT_SYSTEM_PROMPT` at assembly time. Genuinely customized prompts are left
untouched. When `DEFAULT_SYSTEM_PROMPT` changes, the outgoing text must be
appended to `LEGACY_SYSTEM_PROMPTS`.

### 12. Deterministic decoding

Commit generation wants reproducibility, not creativity. OpenAI-compatible and
Gemini bodies already pinned `temperature: 0`; the Anthropic body now does too
(it previously inherited the API default of 1.0).

## Sources

- Anthropic — Prompting best practices:
  <https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices>
- Anthropic — Effective context engineering for AI agents:
  <https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents>
- OpenAI — Best practices for prompt engineering:
  <https://help.openai.com/en/articles/6654000-best-practices-for-prompt-engineering-with-the-openai-api>
- OpenAI — GPT-5 prompting guide:
  <https://developers.openai.com/cookbook/examples/gpt-5/gpt-5_prompting_guide>
- An Empirical Study on Commit Message Generation using LLMs via In-Context
  Learning: <https://arxiv.org/abs/2502.18904>
- Automated Commit Message Generation with Large Language Models — An Empirical
  Study and Beyond: <https://arxiv.org/abs/2404.14824>
- Liu et al., Lost in the Middle — How Language Models Use Long Contexts:
  <https://arxiv.org/abs/2307.03172>
- Conventional Commits 1.0.0: <https://www.conventionalcommits.org/en/v1.0.0/>
- Gitmoji: <https://gitmoji.dev/>
