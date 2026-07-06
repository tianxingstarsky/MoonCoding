/// System prompt 分层构造器。
/// 扩展: Phase C 加 `.with_vector_context(embeddings)`; Phase B 加 ProjectTree 注入。
pub struct PromptBuilder {
    pub personality: String,
    pub project_instructions: String,
    pub tool_descriptions: String,
    pub session_context: String,
    /// Phase B: 树形项目当前状态注入
    pub tree_summary: String,
    /// Phase C: 向量引导注入 (海量范例)
    pub vector_guidance: String,
}

impl PromptBuilder {
    pub fn new(personality: &str) -> Self {
        Self {
            personality: personality.to_string(),
            project_instructions: String::new(),
            tool_descriptions: String::new(),
            session_context: String::new(),
            tree_summary: String::new(),
            vector_guidance: String::new(),
        }
    }

    pub fn with_tools(mut self, descriptions: &str) -> Self {
        self.tool_descriptions = descriptions.to_string();
        self
    }

    pub fn with_session_context(mut self, ctx: &str) -> Self {
        self.session_context = ctx.to_string();
        self
    }

    pub fn with_tree_summary(mut self, summary: &str) -> Self {
        self.tree_summary = summary.to_string();
        self
    }

    pub fn with_vector_guidance(mut self, guidance: &str) -> Self {
        self.vector_guidance = guidance.to_string();
        self
    }

    /// 组装最终 system prompt
    pub fn build(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.push(self.personality.clone());

        if !self.project_instructions.is_empty() {
            parts.push(format!("## Project Instructions\n{}", self.project_instructions));
        }
        if !self.tree_summary.is_empty() {
            parts.push(format!("## Project Tree (Current)\n{}", self.tree_summary));
        }
        if !self.tool_descriptions.is_empty() {
            parts.push(format!("## Available Tools\n{}", self.tool_descriptions));
        }
        if !self.vector_guidance.is_empty() {
            parts.push(format!("## Engineering Patterns & Examples\n{}", self.vector_guidance));
        }
        if !self.session_context.is_empty() {
            parts.push(format!("## Working Context\n{}", self.session_context));
        }
        parts.join("\n\n")
    }
}

/// 从 prompts/agent.md 加载人格基板, 文件不存在则用内嵌默认
pub fn load_personality() -> String {
    if let Ok(content) = std::fs::read_to_string("prompts/agent.md") {
        return content;
    }
    // 内置默认 (精简版)
    r#"You are an AI engineering agent. You read code, search codebases, edit code,
and run tests to complete software engineering tasks for the user.

## Core rules
- Prefer the smallest possible change.
- Verify after every edit. Search before guessing.
- You NEVER edit files by hand (sed/nano/vi) — only through provided tools.
- Use the available search tools (grep/glob) to understand the codebase.

## Code editing
Code edits go through the vibe block-set protocol. The vibe tool handles all
block operations (new/split/insert/replace/drop/assemble/verify).
When editing, prefer whole-block replacement — it is the safer choice.

## Task tracking
Use the todowrite tool to create structured task lists. Break large tasks into
sub-tasks using parent_id for hierarchical planning. Update status as you progress.

## Error handling
When the vibe tool reports "ERR: rev stale", re-run `vibe overview <path>` to get
the current rev and retry. When "cross-block dep impact" warnings appear, check
the affected blocks.

## Shell
Your shell is PowerShell on Windows. Use single-quoted strings for literal text.
The bash tool runs from the workspace directory. You are already there — never use cd.

## Communication
Be concise. Report what you did and whether it succeeded.
Use todowrite to maintain visible progress."#.to_string()
}