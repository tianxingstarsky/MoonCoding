use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolResult};

/// 纯内存 TODO 列表 —— 比 opencode 的 todowrite 多一个树雏形:
/// 每个 item 可以有 parent_id 建立层级关系 (为 Phase B 树形项目做准备)
pub static TODOS: std::sync::LazyLock<std::sync::Mutex<Vec<TodoItem>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoItem {
    pub id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub content: String,
    pub status: String,  // pending | in_progress | completed | cancelled
    pub priority: String, // high | medium | low
}

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str { "todowrite" }
    fn description(&self) -> &str {
        "Create and maintain a structured task list. Tracks progress, organizes multi-step work. \
         Supports hierarchical tasks via parent_id. Use proactively when the task requires 3+ distinct steps."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {"type": "string"},
                            "status": {"type": "string", "enum": ["pending", "in_progress", "completed", "cancelled"]},
                            "priority": {"type": "string", "enum": ["high", "medium", "low"]},
                            "parent_id": {"type": "string", "description": "optional parent task id for hierarchy"}
                        },
                        "required": ["content", "status", "priority"]
                    }
                }
            },
            "required": ["todos"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let _ = ctx;
        let arr = match args.get("todos").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return ToolResult { output: "todos array required".into(), exit_code: 1, duration_ms: 0, truncated: false },
        };

        let mut guard = TODOS.lock().unwrap();
        guard.clear();
        let mut count = 0;
        for item in arr {
            let id = item.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
                .unwrap_or_else(|| format!("task_{}", count));
            let parent_id = item.get("parent_id").and_then(|v| v.as_str()).map(|s| s.to_string());
            let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("pending").to_string();
            let priority = item.get("priority").and_then(|v| v.as_str()).unwrap_or("medium").to_string();
            guard.push(TodoItem { id, parent_id, content, status, priority });
            count += 1;
        }

        let summary = render_tree(&guard);
        ToolResult { output: summary, exit_code: 0, duration_ms: 0, truncated: false }
    }
}

fn render_tree(items: &[TodoItem]) -> String {
    let mut out = String::new();
    let roots: Vec<&TodoItem> = items.iter().filter(|i| i.parent_id.is_none()).collect();
    for r in roots {
        render_node(items, r, 0, &mut out);
    }
    let counts = status_counts(items);
    out.push_str(&format!("\n  pending:{} in_progress:{} completed:{} cancelled:{}",
        counts[0], counts[1], counts[2], counts[3]));
    out
}

fn render_node(items: &[TodoItem], node: &TodoItem, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let mark = match node.status.as_str() {
        "completed" => "\u{2713}",
        "in_progress" => "\u{25b6}",
        "cancelled" => "\u{2717}",
        _ => "\u{25cb}",
    };
    out.push_str(&format!("{} {} [{}|{}] {}\n", indent, mark, node.status, node.priority, node.content));
    let children: Vec<&TodoItem> = items.iter()
        .filter(|i| i.parent_id.as_deref() == Some(&node.id))
        .collect();
    for c in children {
        render_node(items, c, depth + 1, out);
    }
}

fn status_counts(items: &[TodoItem]) -> [usize; 4] {
    let mut c = [0usize; 4];
    for i in items {
        match i.status.as_str() {
            "pending" => c[0] += 1,
            "in_progress" => c[1] += 1,
            "completed" => c[2] += 1,
            "cancelled" => c[3] += 1,
            _ => c[0] += 1,
        }
    }
    c
}

/// 获取当前 TODO 列表的文本摘要 (用于 system prompt 注入)
pub fn current_summary() -> String {
    let guard = TODOS.lock().unwrap();
    if guard.is_empty() { return String::new(); }
    format!("Current task list:\n{}", render_tree(&guard))
}