# vibe-agent Architecture

## 总览

vibe-agent 是 MoonCoding 的 L2 交互智能体核心。它是一个**纯文本 CLI 代理**, 内置六个扩展槽,
后续所有的工程化功能(TUI、HTTP API、树形项目管理、向量引导)都通过扩展现有槽来实现,
**不会改动核心循环**。

## 数据流

```
             ┌─────────────────┐
             │   main.rs       │ CLI 入口: chat / list / resume / tree
             │   (clap parse)  │
             └───────┬─────────┘
                     │
            ┌────────▼─────────┐
            │   agent.rs       │ run_agent(user_input, session, tools, provider)
            │   (核心循环)     │ → Stream<AgentEvent>
            └──┬───────┬───────┘
               │       │
      ┌────────▼──┐ ┌──▼──────────┐
      │ provider  │ │ tools/       │
      │ (LLM SSE) │ │ mod.rs       │ Registry
      └───────────┘ │ ├── bash    │
                    │ ├── read    │
                    │ ├── grep    │
                    │ ├── glob    │
                    │ ├── todowrite│
                    │ └── vibe    │
                    └─────────────┘
```

## 核心循环 (agent.rs)

```rust
async fn run_agent(
    provider: &dyn Provider,
    tools: &ToolRegistry,
    session: &mut Session,
    user_input: &str,
    on_event: &mut dyn FnMut(AgentEvent),
) -> Result<()>
```

开放式回复: 调用 provider → 解析 tool_calls → dispatch tools → 循环(到 done/max_steps/prune)

### 停止条件:
- LLM 返回 `finish_reason=stop` 且无 tool_call
- `step >= max_steps`
- 用户中断 (Ctrl+C)
- 不可恢复错误

### 两端一致性:
- 不做退火/历史压缩 (那是 Phase C 的事)
- 当前只做简单的 token 统计和步骤计数

## 扩展槽详解

### 1. Provider (provider.rs)
Trait: `chat_stream(messages, tools, callback) → (in_tok, out_tok)`
内置: OpenAiCompatible — 所有 OpenAI 兼容端点共用
Ollama、Groq、DeepSeek 都走此路径, 只需 base_url + model 不同
环境变量: MOONCODING_BASE_URL / MOONCODING_MODEL / MOONCODING_API_KEY

### 2. Tool (tools/mod.rs)
Trait: `name / description / parameters (JSON Schema) / execute (args, ctx) → ToolResult`
注册: `registry.register(MyTool)`
所有工具共享 ToolContext { workspace, vibe_exe, session_id }

### 3. AgentEvent (stream.rs)
枚举: Thinking / TextDelta / TextDone / ToolCallStart / ToolCallResult / Done / TreeUpdated / Error / Interrupted
CLI 消费: 匹配事件 → print 格式化行
HTTP 消费: 匹配事件 → json SSE 推

### 4. Session (session.rs)
Trait SessionStore: load / save / list / latest
SqliteStore 实现 (rusqlite bundled)
Expansion: Postgres, JSONL, in-memory

Session struct: id, provider, model, messages, step, tokens, project_tree, metadata

### 5. ProjectTree (session.rs Session.project_tree)
Phase A: Option::None (预留字段)
Phase B: TreeTool implements Tool, 与 agent 共用同一 Session.project_tree
CLI: `vibe-agent tree show/add/mark/review/focus`

### 6. Prompt (prompt.rs)
PromptBuilder: personality + project_instructions + tool_descriptions + session_context
扩展: `.with_vector_context(embeddings)` — Phase C

## 配置

三层覆盖:
1. 环境变量: MOONCODING_BASE_URL / MOONCODING_MODEL / MOONCODING_API_KEY
2. .mooncoding.toml (当前目录或 ~/.config/mooncoding/)
3. CLI flags: --provider / --model / --base-url / --api-key

## 调试

每个模块都可独立调试:
- provider: `cargo test provider` + curl 手动 SSE
- tools: `cargo test tools` 各 tool 独立单元测试
- agent: mock provider + assert AgentEvent 顺序
- config: `println!("{:?}", Config::load())` 快速检查

## 文件依赖图

```
main.rs
├── config.rs (无内部依赖)
├── agent.rs
│   ├── provider.rs
│   ├── tools/mod.rs → tools/{bash,read,grep,glob,todowrite,vibe}.rs
│   ├── session.rs
│   ├── prompt.rs
│   └── stream.rs
