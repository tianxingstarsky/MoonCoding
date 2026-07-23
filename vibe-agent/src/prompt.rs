/// System prompt 分层构造器。
/// ProjectTree 与本地向量工程知识均通过独立层注入。
pub struct PromptBuilder {
    pub personality: String,
    pub language: String,
    pub project_instructions: String,
    pub tool_descriptions: String,
    pub session_context: String,
    /// Phase B: 树形项目当前状态注入
    pub tree_summary: String,
    /// Phase C: 与当前请求相关的向量检索片段
    pub vector_guidance: String,
    /// Host / toolchain facts for the current process
    pub runtime_env: String,
}

impl PromptBuilder {
    pub fn new(personality: &str) -> Self {
        Self {
            personality: personality.to_string(),
            language: String::new(),
            project_instructions: String::new(),
            tool_descriptions: String::new(),
            session_context: String::new(),
            tree_summary: String::new(),
            vector_guidance: String::new(),
            runtime_env: String::new(),
        }
    }

    pub fn with_language(mut self, lang: &str) -> Self {
        self.language = lang.to_string();
        self
    }

    pub fn with_tools(mut self, descriptions: &str) -> Self {
        self.tool_descriptions = descriptions.to_string();
        self
    }

    pub fn with_project_instructions(mut self, instructions: &str) -> Self {
        self.project_instructions = instructions.to_string();
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

    pub fn with_runtime_env(mut self, env: &str) -> Self {
        self.runtime_env = env.to_string();
        self
    }

    /// 组装最终 system prompt
    pub fn build(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.push(self.personality.clone());

        // App-building constraints — always injected so the agent knows the rules.
        parts.push(APP_CONSTRAINTS.to_string());
        parts.push(RUNTIME_AND_COMPLETION.to_string());
        parts.push(BOARD_DEPLOYMENT.to_string());

        if !self.runtime_env.is_empty() {
            parts.push(format!("## Current host facts\n{}", self.runtime_env));
        }

        if !self.language.is_empty() {
            let lang_instruction = match self.language.as_str() {
                "zh" => "## Communication language\nAlways respond in Chinese (Simplified). Write all descriptions, comments, node notes, and code comments in Chinese. Only code identifiers and technical terms may stay in English.",
                _ => "## Communication language\nRespond in English.",
            };
            parts.push(lang_instruction.to_string());
        }

        if !self.project_instructions.is_empty() {
            parts.push(format!(
                "## Project Instructions\n{}",
                self.project_instructions
            ));
        }
        if !self.tree_summary.is_empty() {
            parts.push(format!("## Project Tree (Current)\n{}", self.tree_summary));
        }
        if !self.tool_descriptions.is_empty() {
            parts.push(format!("## Available Tools\n{}", self.tool_descriptions));
        }
        if !self.vector_guidance.is_empty() {
            parts.push(format!(
                "## Engineering Patterns & Examples\n{}",
                self.vector_guidance
            ));
        }
        if !self.session_context.is_empty() {
            parts.push(format!("## Working Context\n{}", self.session_context));
        }
        parts.join("\n\n")
    }
}

/// Fixed product/toolchain facts + completion criteria (not host-specific paths).
const RUNTIME_AND_COMPLETION: &str = r#"## Product & toolchain facts
- MoonCoding GUI is **Qt 6 / C++** on desktop and board (same `vibe-ui` tree).
- Each **project = one independent workspace folder**. Never read or modify
  files from another project. If the workspace looks empty, CREATE new files —
  do not hunt for leftover apps/calculator or sibling folders.
- Agent edits files with OpenCode-style tools: `read` / `write` / `grep` / `glob`
  / `verify_command`. Allowed write extensions: **html, css, js, py** only.
- Legacy **vibe block** editing is deprecated (disabled unless MOONCODING_ENABLE_VIBE=1).
  Do not call `vibe` or `apps` tools in normal workflows.

## Debugging & "done" criteria
1. Ship a working `index.html` at the project root (required entry).
2. Prove UI/backend with `verify_command` when useful (e.g. `python -m py_compile backend.py`).
3. Mark tree nodes `completed` only with real exit-0 evidence from this workspace.
4. If stuck, set `failed`/`needs_review`, explain briefly, and stop.
"#;

/// Board full-product deployment (Lyra / linuxfb).
const BOARD_DEPLOYMENT: &str = r#"## Board / phone UI (Luckfox Lyra · 720×1280 portrait)
- Physical panel is **native portrait 720×1280**. Design for a **phone**:
  single-column layout, large touch targets (≥44px), readable 16px+ text,
  no desktop multi-column dashboards, no hover-only interactions.
- Stack: Qt 6 + linuxfb; project preview prefers WebEngine when available,
  otherwise HTML preview in the host.
- LLM uses OpenAI-compatible cloud API. Keep pages light (few images, no CDN frameworks).
- Cross-build/deploy notes only matter when the user asks to ship firmware/binaries.
"#;

/// Product application model — one Web app per project.
const APP_CONSTRAINTS: &str = r#"## Project application model (ONE app per project)

### Hard rules
1. **Workspace isolation**: You may ONLY touch the current workspace path from
   host facts. Creating a new project means an empty folder — start from
   `index.html`, do **not** open or "optimize" another project's files.
2. **Single entry**: `index.html` at the project root is the only UI entry.
   Do not create `apps/<name>/` multi-app packages for new work.
3. **Allowed files you may write**: `.html` `.css` `.js` `.py` only.
4. **Prefer pure front-end**: HTML + CSS + JS. Use Python only when unavoidable
   (local compute, GPIO later, etc.).

### Portrait phone layout (mandatory)
- Viewport: treat as **720×1280 CSS pixels**, `width=device-width`.
- Use vertical stacks, full-width buttons, generous padding.
- Example meta: `<meta name="viewport" content="width=device-width, initial-scale=1">`
- Avoid `position:fixed` footers that cover content; keep one screenful of primary actions.

### Suggested file layout
```
index.html      # required entry
styles.css      # optional
app.js          # optional
backend.py      # optional Python helper (not the UI entry)
```

### Python backend (optional) — host-managed lifecycle
If `backend.py` exists at the project root, the Apps host **auto-starts** it when
opening preview (no start button required). The process is **project-scoped**:
it may keep running while the user switches Chat/Tree (background OK). It is
**destroyed immediately** when switching projects, closing the workspace, or on
explicit stop — so the port is freed.

Host contract:
- Port is assigned by the host from the workspace path (`MOONCODING_BACKEND_PORT`,
  `MOONCODING_BACKEND_HOST=127.0.0.1`, `MOONCODING_API_BASE=http://127.0.0.1:<port>`).
- Prefer binding that host/port. Print one line `READY <url-or-message>` on stdout when usable.
- Preview **injects** `window.__MOONCODING_API_BASE__` before page scripts. Do **not**
  invent ports or hardcode `localhost:8000`. Front-end must use the injected base.
- Optional links `mooncoding://backend/start|stop` still work as manual overrides.
- Use stdlib only (no pip).

### Minimal scaffolds (copy this pattern)

`backend.py` (binds host port only):
```python
import json, os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
HOST = os.environ.get("MOONCODING_BACKEND_HOST", "127.0.0.1")
PORT = int(os.environ.get("MOONCODING_BACKEND_PORT", "18765"))
API = os.environ.get("MOONCODING_API_BASE", f"http://{HOST}:{PORT}")
class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        body = json.dumps({"ok": True, "api_base": API}).encode()
        self.send_response(200); self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
print(f"READY {API}", flush=True)
ThreadingHTTPServer((HOST, PORT), H).serve_forever()
```

`app.js` (always read injected base):
```javascript
function apiBase() {
  return (typeof window.__MOONCODING_API_BASE__ === "string" && window.__MOONCODING_API_BASE__)
    || "";
}
fetch(apiBase() + "/").then(r => r.json()).then(console.log).catch(console.error);
```

LLM **must** call tool `preview_backend` with `action=stop` before editing/replacing
`backend.py`, when the port is busy, or before finishing work that leaves a live
server you no longer need. Writing `backend.py` while it is running is **refused**.
Use `action=status` / `action=ensure` to inspect or start from tools.
Without `backend.py`, the front-end alone must work.

### Do NOT
- Invent multi-app sidebars, `ui.json` native schemas, or vibe block workflows.
- Call `App()` from mooncoding_app at import time (legacy SDK hang).
- Copy calculator/demo code from a previous workspace.
- Use npm/CDN/React/Vue for board apps — keep vanilla HTML/CSS/JS.
"#;

pub fn load_personality() -> String {
    include_str!("../prompts/agent.md").to_string()
}

pub fn load_project_instructions(root: &std::path::Path) -> String {
    const MAX_INSTRUCTION_BYTES: usize = 16_000;
    let path = root.join("AGENTS.md");
    let Ok(mut instructions) = std::fs::read_to_string(path) else {
        return String::new();
    };
    if instructions.len() > MAX_INSTRUCTION_BYTES {
        let mut truncate_at = MAX_INSTRUCTION_BYTES;
        while truncate_at > 0 && !instructions.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }
        instructions.truncate(truncate_at);
        instructions.push_str("\n\n[Project instructions truncated by MoonCoding]");
    }
    instructions
}

/// Host-specific facts injected each run (paths, OS, vibe binary).
pub fn build_runtime_env(cfg: &crate::config::Config) -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let target = cfg.deployment_target.as_str();
    let board_hint = if cfg.deployment_target == crate::config::DeploymentTarget::Board {
        "\n- Board mode: full Qt6 GUI via linuxfb; cross-build `build-board/`; \
         launch `./mooncoding -platform linuxfb --ui-profile 720p`"
    } else {
        ""
    };
    format!(
        "- OS: {os}/{arch}\n\
         - ACTIVE WORKSPACE (only folder you may touch): {}\n\
         - Isolation: never open sibling projects or /root/mooncoding-ws leftovers\n\
         - Project entry: index.html (create it if missing)\n\
         - UI framework for host shell: Qt 6\n\
         - deployment_target: {target}\n\
         - Session store: {}\n\
         - Prefer workspace-relative paths in tools{board_hint}",
        cfg.workspace.display(),
        cfg.session_dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_layers_keep_human_tree_context_separate() {
        let prompt = PromptBuilder::new("personality")
            .with_project_instructions("project rule")
            .with_tree_summary("human_locked")
            .with_tools("tree")
            .with_runtime_env("OS: windows")
            .build();
        assert!(prompt.contains("## Project Instructions\nproject rule"));
        assert!(prompt.contains("## Project Tree (Current)\nhuman_locked"));
        assert!(prompt.contains("## Available Tools\ntree"));
        assert!(prompt.contains("## Current host facts\nOS: windows"));
        assert!(prompt.contains("Qt 6"));
        assert!(prompt.contains("## Board deployment"));
        assert!(prompt.contains("linuxfb"));
        assert!(!prompt.contains("Qt 5 only"));
        assert!(!prompt.contains("kiosk-only shell as the product"));
    }

    #[test]
    fn board_runtime_env_mentions_linuxfb() {
        let cfg = crate::config::Config {
            language: "zh".into(),
            api_source: crate::config::ApiSource::Custom,
            managed_api: None,
            provider: crate::config::ProviderConfig {
                base_url: "https://example.com".into(),
                model: "m".into(),
                api_key: String::new(),
                max_tokens: 1024,
                temperature: 0.1,
            },
            agent: crate::config::AgentToml::default(),
            workspace: std::path::PathBuf::from("/tmp/ws"),
            vibe_exe: std::path::PathBuf::from("vibe"),
            session_dir: std::path::PathBuf::from("/tmp/ws/.mooncoding/sessions"),
            deployment_target: crate::config::DeploymentTarget::Board,
        };
        let env = build_runtime_env(&cfg);
        assert!(env.contains("deployment_target: board"));
        assert!(env.contains("linuxfb"));
        assert!(env.contains("720p"));
    }
}
