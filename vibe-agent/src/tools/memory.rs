use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolResult};

pub struct MemoryTool;

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Search or persist project-specific engineering knowledge. Remember only durable \
         architecture decisions, verified debugging lessons, or reusable project conventions; \
         never store secrets, temporary progress, or unverified guesses."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["search", "remember"]},
                "query": {"type": "string"},
                "title": {"type": "string"},
                "content": {"type": "string"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "limit": {"type": "integer", "minimum": 1, "maximum": 10}
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let action = args.get("action").and_then(Value::as_str).unwrap_or("");
        let result = match action {
            "search" => search(&args, ctx),
            "remember" => remember(&args, ctx),
            _ => Err(anyhow::anyhow!("unknown memory action: {action}")),
        };
        match result {
            Ok(output) => ToolResult {
                output,
                exit_code: 0,
                duration_ms: 0,
                truncated: false,
            },
            Err(error) => ToolResult {
                output: error.to_string(),
                exit_code: 1,
                duration_ms: 0,
                truncated: false,
            },
        }
    }
}

fn search(args: &Value, ctx: &ToolContext) -> anyhow::Result<String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .filter(|query| !query.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("query is required"))?;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 10) as usize;
    let knowledge = ctx
        .knowledge
        .read()
        .map_err(|_| anyhow::anyhow!("knowledge base lock poisoned"))?;
    let hits: Vec<Value> = knowledge
        .search(query, limit)
        .into_iter()
        .map(|hit| {
            json!({
                "id": hit.entry.id,
                "title": hit.entry.title,
                "content": hit.entry.content,
                "source": hit.entry.source,
                "tags": hit.entry.tags,
                "score": hit.score
            })
        })
        .collect();
    Ok(json!({"hits": hits}).to_string())
}

fn remember(args: &Value, ctx: &ToolContext) -> anyhow::Result<String> {
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("title is required"))?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("content is required"))?;
    let tags = args
        .get("tags")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    if looks_sensitive(&format!("{title}\n{content}\n{}", tags.join(" "))) {
        anyhow::bail!("knowledge rejected because it appears to contain a secret");
    }
    let mut knowledge = ctx
        .knowledge
        .write()
        .map_err(|_| anyhow::anyhow!("knowledge base lock poisoned"))?;
    let id = knowledge.remember(title, content, tags, "ai")?;
    Ok(json!({"remembered_id": id}).to_string())
}

fn looks_sensitive(content: &str) -> bool {
    let lowercase = content.to_ascii_lowercase();
    [
        "api_key=",
        "api key:",
        "authorization: bearer",
        "private key-----",
        "password=",
        "secret=",
    ]
    .iter()
    .any(|marker| lowercase.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::looks_sensitive;

    #[test]
    fn rejects_common_secret_shapes() {
        assert!(looks_sensitive("Authorization: Bearer abc"));
        assert!(looks_sensitive("password=hunter2"));
        assert!(!looks_sensitive(
            "Never log API credentials in provider diagnostics."
        ));
    }
}
