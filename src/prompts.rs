use crate::commands::config::Config;
use crate::engine::engine::Message;
use crate::error::Result;
use crate::i18n::get_translation;

// Identity for the AI assistant
const IDENTITY: &str = "You are to act as an author of a commit message in git.";

// GitMoji help text
const GITMOJI_HELP: &str = 
"Use GitMoji convention to preface the commit. Here are some help to choose the right emoji (emoji, description): 
🐛, Fix a bug; 
✨, Introduce new features; 
📝, Add or update documentation; 
🚀, Deploy stuff; 
✅, Add, update, or pass tests; 
♻️, Refactor code; 
⬆️, Upgrade dependencies; 
🔧, Add or update configuration files; 
🌐, Internationalization and localization; 
💡, Add or update comments in source code;";

// Full GitMoji specification
const FULL_GITMOJI_SPEC: &str = 
"Use GitMoji convention to preface the commit. Here are all the available emoji options:
🎨, Improve structure / format of the code; 
⚡️, Improve performance; 
🔥, Remove code or files; 
🐛, Fix a bug; 
🚑️, Critical hotfix; 
✨, Introduce new features; 
📝, Add or update documentation; 
🚀, Deploy stuff; 
💄, Add or update the UI and style files; 
🎉, Begin a project; 
✅, Add, update, or pass tests; 
🔒️, Fix security issues; 
🔐, Add or update secrets; 
🔖, Release / Version tags; 
🚨, Fix compiler / linter warnings; 
🚧, Work in progress; 
💚, Fix CI Build; 
⬇️, Downgrade dependencies; 
⬆️, Upgrade dependencies; 
📌, Pin dependencies to specific versions; 
👷, Add or update CI build system; 
📈, Add or update analytics or track code; 
➕, Add a dependency; 
➖, Remove a dependency; 
🔧, Add or update configuration files; 
🔨, Add or update development scripts; 
✏️, Fix typos; 
💩, Write bad code that needs to be improved; 
⏪️, Revert changes; 
🔀, Merge branches; 
📦️, Add or update compiled files or packages; 
👽️, Update code due to external API changes; 
🚚, Move or rename resources (e.g.: files, paths, routes); 
📄, Add or update license; 
💥, Introduce breaking changes; 
🍱, Add or update assets; 
♿️, Improve accessibility; 
💬, Add or update text and literals; 
🗃️, Perform database related changes; 
🔊, Add or update logs; 
🔇, Remove logs; 
👥, Add or update contributor(s); 
🚸, Improve user experience / usability; 
🏗️, Make architectural changes; 
📱, Work on responsive design; 
🤡, Mock things; 
🥚, Add or update an easter egg; 
🙈, Add or update a .gitignore file; 
📸, Add or update snapshots; 
⚗️, Perform experiments; 
🔍️, Improve SEO; 
🏷️, Add or update types; 
🌱, Add or update seed files; 
🚩, Add, update, or remove feature flags; 
🥅, Catch errors; 
💫, Add or update animations and transitions; 
🗑️, Deprecate code that needs to be cleaned up; 
🛂, Work on code related to authorization, roles and permissions; 
🩹, Simple fix for a non-critical issue; 
🧐, Data exploration/inspection; 
⚰️, Remove dead code; 
🧪, Add a failing test; 
👔, Add or update business logic; 
🩺, Add or update healthcheck; 
🧱, Infrastructure related changes; 
🧑‍💻, Improve developer experience; 
💸, Add sponsorships or money related infrastructure; 
🧵, Add or update code related to multithreading or concurrency; 
🦺, Add or update code related to validation.";

// Conventional commit keywords
const CONVENTIONAL_COMMIT_KEYWORDS: &str = 
"Do not preface the commit with anything, except for the conventional commit keywords: fix, feat, build, chore, ci, docs, style, refactor, perf, test.";

// Example diff for consistency
const INIT_DIFF: &str = 
"diff --git a/src/server.ts b/src/server.ts
index ad4db42..f3b18a9 100644
--- a/src/server.ts
+++ b/src/server.ts
@@ -10,7 +10,7 @@
import {
    initWinstonLogger();
    
    const app = express();
    -const port = 7799;
    +const PORT = 7799;
    
    app.use(express.json());
    
    @@ -34,6 +34,6 @@
    app.use((_, res, next) => {
        // ROUTES
        app.use(PROTECTED_ROUTER_URL, protectedRouter);
        
        -app.listen(port, () => {
            -  console.log(\`Server listening on port \${port}\`);
            +app.listen(process.env.PORT || PORT, () => {
                +  console.log(\`Server listening on port \${PORT}\`);
            });";

// Get main prompt for commit message generation
pub async fn get_main_commit_prompt(full_gitmoji_spec: bool, context: String) -> Result<Vec<Message>> {
    let config = Config::load()?;
    let translation = get_translation(&config.language)?;
    
    // Determine emoji/convention guidance
    let commit_convention = if config.emoji {
        if full_gitmoji_spec {
            FULL_GITMOJI_SPEC
        } else {
            GITMOJI_HELP
        }
    } else {
        CONVENTIONAL_COMMIT_KEYWORDS
    };
    
    // Determine description guidance
    let description_guidance = if config.description {
        "Add a short description of WHY the changes are done after the commit message. Don't start it with \"This commit\", just describe the changes."
    } else {
        "Don't add any descriptions to the commit, only commit message."
    };
    
    // Determine one-line commit guidance
    let one_line_guidance = if config.one_line_commit {
        "Craft a concise commit message that encapsulates all changes made, with an emphasis on the primary updates. If the modifications share a common theme or scope, mention it succinctly; otherwise, leave the scope out to maintain focus. The goal is to provide a clear and unified overview of the changes in a one single message, without diverging into a list of commit per file change."
    } else {
        ""
    };
    
    // User context if provided
    let user_context = if !context.is_empty() {
        format!("Additional context provided by the user: <context>{}</context>\nConsider this context when generating the commit message, incorporating relevant information when appropriate.", context)
    } else {
        String::new()
    };
    
    // System message content
    let system_content = format!(
        "{} Your mission is to create clean and comprehensive commit messages and explain WHAT were the changes {}.\n\
        I'll send you an output of 'git diff --staged' command, and you are to convert it into a commit message.\n\
        {}\n\
        {}\n\
        {}\n\
        Use the present tense. Lines must not be longer than 74 characters. Use {} for the commit message.\n\
        {}",
        IDENTITY,
        if config.why { "and WHY the changes were done" } else { "" },
        commit_convention,
        description_guidance,
        one_line_guidance,
        translation.local_language,
        user_context
    );
    
    // Create consistency message with examples
    let consistency_content = if config.emoji {
        format!(
            "🐛 {}\n✨ {}\n{}",
            translation.commit_fix.replacen("fix", "", 1).trim_start(),
            translation.commit_feat.replacen("feat", "", 1).trim_start(),
            if config.description { &translation.commit_description } else { "" }
        )
    } else {
        format!(
            "{}\n{}\n{}",
            translation.commit_fix,
            translation.commit_feat,
            if config.description { &translation.commit_description } else { "" }
        )
    };
    
    // Create messages
    let mut messages = Vec::new();
    
    // Add system message
    messages.push(Message::system(system_content));
    
    // Add example diff (will be replaced with actual diff when calling the API)
    messages.push(Message::user(INIT_DIFF));
    
    // Add assistant example response for consistency
    messages.push(Message::assistant(consistency_content));
    
    Ok(messages)
}

// Generate prompt for commitlint consistency
pub async fn get_commitlint_consistency_prompt(prompts: &[String]) -> Result<Vec<Message>> {
    let config = Config::load()?;
    let translation = get_translation(&config.language)?;
    
    // Create system message with prompts
    let prompts_text = prompts.iter()
        .map(|p| format!("- {}", p))
        .collect::<Vec<_>>()
        .join("\n");
    
    let system_content = format!(
        "{} Your mission is to create clean and comprehensive commit messages for two different changes in a single codebase and output them in the provided JSON format: one for a bug fix and another for a new feature.

Here are the specific requirements and conventions that should be strictly followed:

Commit Message Conventions:
- The commit message consists of three parts: Header, Body, and Footer.
- Header: 
  - Format: `<type>(<scope>): <subject>`
{prompts_text}

JSON Output Format:
- The JSON output should contain the commit messages for a bug fix and a new feature in the following format:
```json
{{
  "localLanguage": "{local_language}",
  "commitFix": "<Header of commit for bug fix>",
  "commitFeat": "<Header of commit for feature>",
  "commitDescription": "<Description of commit for both the bug fix and the feature>"
}}