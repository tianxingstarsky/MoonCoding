use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::provider::ToolDef;

pub mod bash;
pub mod glob;
pub mod grep;
pub mod read;
pub mod todowrite;
pub mod vibe;

/// 工具执行上下文——传入当前工作目录、vibe 二进制路径、session 信息
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub workspace: std::path::PathBuf,
    pub vibe_exe: std::path::PathBuf,
    pub session_id: String,
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
    fn parameters(&self) -> Value;  // JSON Schema
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult;
}

/// 工具注册表 —— 将来的 lsp / git / tree / diff 全在这里加
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new(), order: Vec::new() }
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        self.order.push(name.clone());
        self.tools.insert(name, Arc::new(tool));
    }

    /// 生成 OpenAI 函数调用格式的 tool 定义列表
    pub fn definitions(&self) -> Vec<ToolDef> {
        self.order.iter().filter_map(|name| {
            let t = self.tools.get(name)?;
            Some(ToolDef {
                r#type: "function".to_string(),
                function: crate::provider::ToolFunction {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    parameters: t.parameters(),
                },
            })
        }).collect()
    }

    /// 按名分发 (async)
    pub async fn dispatch(&self, name: &str, args: Value, ctx: &ToolContext) -> Option<ToolResult> {
        if let Some(t) = self.tools.get(name) {
            Some(t.execute(args, ctx).await)
        } else {
            None
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self { Self::new() }
}