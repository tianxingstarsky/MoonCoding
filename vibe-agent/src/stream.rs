use serde::{Deserialize, Serialize};

use crate::app_runtime::AppRuntimeEvent;

/// Agent 状态变更事件——CLI 终端消费它画到 stdout, 未来 HTTP 消费它转 SSE.
/// 这是你说的"CLI 做核心, UI 套壳"的桥接层。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    /// 开始新一轮思考
    Thinking,
    /// 模型 reasoning / 思考链增量（DeepSeek reasoning_content 等）
    ThinkingDelta(String),
    /// 流式文本增量(token-level)
    TextDelta(String),
    /// 完整 assistant 消息 (一次 tool-use round 结束)
    TextDone {
        content: String,
        tokens_in: u64,
        tokens_out: u64,
    },
    /// 工具调用开始
    ToolCallStart {
        id: String,
        name: String,
        input: String,
    },
    /// 工具调用结果
    ToolCallResult {
        id: String,
        name: String,
        output: String,
        exit_code: i32,
        duration_ms: u64,
    },
    /// 本轮结束
    Done {
        tokens_in: u64,
        tokens_out: u64,
        steps: u64,
    },
    /// 持久项目树更新，Qt 收到后原子替换 TreeModel
    TreeUpdated { json: String },
    /// 微应用运行时事件（不经过 Agent 聊天循环）
    AppRuntime(AppRuntimeEvent),
    /// 错误
    Error(String),
    /// 人工中断 / limit hit / 不可恢复
    Interrupted(String),
}
