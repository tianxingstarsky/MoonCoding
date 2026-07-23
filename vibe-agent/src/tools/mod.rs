use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::app_runtime::AppRuntimeManager;
use crate::provider::ToolDef;
use crate::tree::TreeManager;
use crate::vector::KnowledgeBase;

pub mod apps;
pub mod bash;
pub mod blockgate;
pub mod glob;
pub mod grep;
pub mod memory;
pub mod paths;
pub mod preview_backend;
pub mod read;
pub mod tree;
pub mod vibe;
pub mod write;

/// 工具执行上下文——传入当前工作目录、vibe 二进制路径、session 信息
#[derive(Clone)]
pub struct ToolContext {
    pub workspace: std::path::PathBuf,
    pub vibe_exe: std::path::PathBuf,
    pub session_id: String,
    pub project_tree: Arc<RwLock<TreeManager>>,
    pub command_log: Arc<RwLock<Vec<CommandExecution>>>,
    pub knowledge: Arc<RwLock<KnowledgeBase>>,
    /// Shared native app runtime sandbox (desktop + LLM control plane).
    pub app_runtime: Option<Arc<AppRuntimeManager>>,
}

#[derive(Debug, Clone)]
pub struct CommandExecution {
    pub command: String,
    pub exit_code: i32,
    pub tool: String,
    pub verification_kind: String,
    pub working_directory: PathBuf,
    pub completed_at: String,
}

/// 工具执行结果
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub output: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub truncated: bool,
}

/// 工具 trait —— 加一个新工具只需 impl 这一接口
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value; // JSON Schema
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult;
}

/// 工具注册表 —— 将来的 lsp / git / tree / diff 全在这里加
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        self.order.push(name.clone());
        self.tools.insert(name, Arc::new(tool));
    }

    /// 生成 OpenAI 函数调用格式的 tool 定义列表
    pub fn definitions(&self) -> Vec<ToolDef> {
        self.order
            .iter()
            .filter_map(|name| {
                let t = self.tools.get(name)?;
                Some(ToolDef {
                    r#type: "function".to_string(),
                    function: crate::provider::ToolFunction {
                        name: t.name().to_string(),
                        description: t.description().to_string(),
                        parameters: t.parameters(),
                    },
                })
            })
            .collect()
    }

    /// 按名分发 (async). `bash` is accepted as an alias of `verify_command`.
    pub async fn dispatch(&self, name: &str, args: Value, ctx: &ToolContext) -> Option<ToolResult> {
        let resolved = if name == "bash" {
            "verify_command"
        } else {
            name
        };
        if let Some(t) = self.tools.get(resolved) {
            Some(t.execute(args, ctx).await)
        } else {
            None
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the standard tool set used by every frontend.
///
/// Frontends must not maintain their own registration list; adding a tool here makes
/// it available to the desktop UI, tests, and any compatibility binary.
///
/// Vibe block editing + multi-app tooling are **deprecated** (code kept). Re-enable with
/// `MOONCODING_ENABLE_VIBE=1` for emergency/legacy workflows only.
pub fn default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(read::ReadTool);
    registry.register(write::WriteTool);
    registry.register(grep::GrepTool);
    registry.register(glob::GlobTool);
    registry.register(bash::BashTool);
    registry.register(memory::MemoryTool);
    registry.register(tree::TreeTool);
    registry.register(preview_backend::PreviewBackendTool);
    let enable_vibe = std::env::var("MOONCODING_ENABLE_VIBE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if enable_vibe {
        registry.register(vibe::VibeTool);
        registry.register(apps::AppsTool);
    }
    registry
}
