use crate::stream::AgentEvent;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

const TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub r#type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// AI 返回的流式工具调用（内部中间态）
pub struct ParsedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// 流式事件（provider-native，泛用的）
pub enum StreamEvent {
    TextDelta(String),
    ToolCallDone(ParsedToolCall),
    Finish { prompt_tokens: u64, completion_tokens: u64 },
}

// ── OpenAI-compatible 协议解析 ──

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}
#[derive(Debug, Deserialize)]
struct Choice {
    #[serde(default)]
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}
#[derive(Debug, Default, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<StreamToolCall>,
}
#[derive(Debug, Deserialize)]
struct StreamToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<StreamFunction>,
}
#[derive(Debug, Deserialize)]
struct StreamFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}
#[derive(Debug, Default, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}


/// OpenAI-compatible provider (DeepSeek / Groq / local ollama / custom)
pub struct OpenAiCompatible {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
    max_tokens: u64,
    temperature: f64,
}

impl OpenAiCompatible {
    pub fn new(
        base_url: String, model: String, api_key: String,
        max_tokens: u64, temperature: f64,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()?;
        Ok(Self { client, base_url, model, api_key, max_tokens, temperature })
    }

    /// 流式对话: 把 messages + tools 发出去, 通过 callback 接收 StreamEvent
    pub async fn chat_stream<F: FnMut(StreamEvent)>(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        mut cb: F,
    ) -> Result<(u64, u64)> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
            "max_tokens": self.max_tokens,
            "temperature": self.temperature,
            "tools": tools,
            "tool_choice": "auto",
        });

        let mut req = self.client.post(&url).json(&body);
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("http {}: {}", status, text));
        }

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut tool_acc: BTreeMap<usize, (String, String, String)> = BTreeMap::new(); // index → (id, name, args)
        let mut prompt_tokens = 0u64;
        let mut completion_tokens = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buf.push_str(&chunk_str);

            while let Some(idx) = buf.find('\n') {
                let line = buf[..idx].trim_end_matches('\r').to_string();
                buf.drain(..=idx);
                if line.is_empty() || !line.starts_with("data: ") { continue; }
                let payload = line["data: ".len()..].trim();
                if payload == "[DONE]" { continue; }
                let parsed: ChatCompletionChunk = match serde_json::from_str(payload) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if let Some(u) = parsed.usage {
                    if u.prompt_tokens > 0 { prompt_tokens = u.prompt_tokens; }
                    if u.completion_tokens > 0 { completion_tokens = u.completion_tokens; }
                }
                for choice in parsed.choices {
                    if let Some(t) = choice.delta.content {
                        cb(StreamEvent::TextDelta(t));
                    }
                    for tc in choice.delta.tool_calls {
                        let entry = tool_acc.entry(tc.index).or_insert_with(|| (String::new(), String::new(), String::new()));
                        if let Some(id) = tc.id { entry.0 = id; }
                        if let Some(f) = tc.function {
                            if let Some(n) = f.name { if !n.is_empty() { entry.1 = n; } }
                            if let Some(a) = f.arguments { entry.2.push_str(&a); }
                        }
                    }
                    if choice.finish_reason.is_some() {
                        let mut keys: Vec<usize> = tool_acc.keys().copied().collect();
                        keys.sort();
                        for k in keys {
                            if let Some((id, name, args)) = tool_acc.remove(&k) {
                                if !name.is_empty() {
                                    cb(StreamEvent::ToolCallDone(ParsedToolCall { id, name, arguments: args }));
                                }
                            }
                        }
                        cb(StreamEvent::Finish { prompt_tokens, completion_tokens });
                    }
                }
            }
        }
        Ok((prompt_tokens, completion_tokens))
    }
}