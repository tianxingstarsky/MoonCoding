use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone)]
pub struct LlmClient {
    pub client: Client,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

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

pub enum StreamEvent {
    TextDelta(String),
    ToolCallDone(ToolCall),
    Finish { prompt_tokens: u64, completion_tokens: u64 },
}

impl LlmClient {
    pub fn new(api_key: String, base_url: String, model: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()?;
        Ok(Self { client, api_key, base_url, model })
    }

    /// 流式对话: 把 messages 发出去, 通过 cb 接收 StreamEvent
    pub async fn chat_stream<F: FnMut(StreamEvent)>(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        mut cb: F,
    ) -> Result<(u64, u64)> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
            "tools": tools,
            "tool_choice": "auto",
        });
        let req = self.client.post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        if !req.status().is_success() {
            let status = req.status();
            let text = req.text().await.unwrap_or_default();
            return Err(anyhow!("http {}: {}", status, text));
        }

        let mut stream = req.bytes_stream();
        let mut buf = String::new();
        let mut tool_acc: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
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
                    Err(_) => continue, // partial / keepalive
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
                    if let Some(fr) = choice.finish_reason {
                        // flush accumulated tool calls for this chunk
                        let mut keys: Vec<usize> = tool_acc.keys().copied().collect();
                        keys.sort();
                        for k in keys {
                            if let Some((id, name, args)) = tool_acc.remove(&k) {
                                if !name.is_empty() {
                                    cb(StreamEvent::ToolCallDone(ToolCall {
                                        id, r#type: "function".to_string(),
                                        function: FunctionCall { name, arguments: args },
                                    }));
                                }
                            }
                        }
                        cb(StreamEvent::Finish { prompt_tokens, completion_tokens });
                        let _ = fr;
                    }
                }
            }
        }
        Ok((prompt_tokens, completion_tokens))
    }
}