use anyhow::Result;

/// 构造 system prompt: "你是 vibe AI"角色卡 + vibe 协议手册(README 内容) + 工具使用规则
pub fn build_system_prompt() -> Result<String> {
    // 优先用 vibe-test/prompts/system_prompt.md (人工精修版)
    let manual = std::fs::read_to_string("prompts/system_prompt.md").ok();
    if let Some(text) = manual {
        return Ok(text);
    }
    // 否则动态把 vibe/README.md 加上角色卡
    let readme = std::fs::read_to_string("../vibe/README.md")
        .or_else(|_| std::fs::read_to_string("vibe/README.md"))
        .unwrap_or_else(|_| "vibe protocol reference not found".to_string());
    let s = format!(
        "You are an AI agent operating the `vibe` CLI to write code autonomously.\n\
         You prefer the smallest possible change. You NEVER edit files with vi/nano/sed directly;\n\
         you ONLY mutate code through the `vibe` commands (new/split/insert/replace/drop/assemble).\n\
         You must verify changes worked before declaring done.\n\n\
         \n\
         === VIBE PROTOCOL MANUAL ===\n\n{}\n\n\
         === END OF MANUAL ===\n\n\
         Tool: `bash`. Input: {{\"command\": <string>, \"workdir\"?: <string>}}.\n\
         All commands run inside the workspace sandbox. When you finish the task,\n\
         emit the literal token `<done/>` on its own line as the LAST part of your reply.\n\
         If a command fails or vibe returns `ERR: rev stale`, read the message,\n\
         re-run `vibe overview <path>` to refresh, and retry. Never leave the loop early.",
        readme
    );
    Ok(s)
}