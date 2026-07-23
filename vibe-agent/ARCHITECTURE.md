# vibe-agent Architecture

## 总览

vibe-agent 是 MoonCoding 的无界面 Rust 后端，以 `rlib + cdylib` 交付。Qt UI 只通过
`include/vibe_agent.h` 中的稳定 C ABI 访问它。legacy CLI 源码保留用于迁移，但默认 feature
关闭、不参与产品构建。新功能继续通过扩展槽实现，不把 UI 状态写入 agent 核心循环。

## 数据流

```
             ┌─────────────────┐
             │ Qt 6 / C++ UI   │ chat + editable project tree
             └───────┬─────────┘
                     │ C ABI + JSON events
             ┌───────▼─────────┐
             │ ffi / desktop   │ lifecycle, async thread, human tree actions
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
      └───────────┘ │ ├── verify_command │
                    │ ├── read    │
                    │ ├── grep    │
                    │ ├── glob    │
                    │ ├── tree    │ persistent tree + human authority
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
所有工具共享 ToolContext { workspace, vibe_exe, session_id, project_tree, command_log }。
`command_log` 记录本轮真实命令和退出码，TreeTool 只接受其中 exit=0 的命令作为 AI 完成证据。

### 3. AgentEvent (stream.rs)
枚举: Thinking / TextDelta / TextDone / ToolCallStart / ToolCallResult / Done / TreeUpdated / Error / Interrupted
Qt 消费: RustBridge 将 JSON callback 排队转发为 Qt signals

### 4. Session (session.rs)
Trait SessionStore: load / save / list / latest
SqliteStore 实现 (rusqlite bundled)
Expansion: Postgres, JSONL, in-memory

Session struct: id, provider, model, messages, step, tokens, project_tree, metadata

### 5. ProjectTree (tree.rs + tools/tree.rs)
- `tree_version` 对每次写操作做 stale-write 防撞。
- 每个节点记录 creator、last modifier、字段级 `human_locked_fields`、关联文件与验证证据。
- AI 不能覆盖人工字段，也不能删除含人工节点/修改的分支。
- AI 标记 completed 前必须有最新成功证据，且证据命令必须在本轮真实 exit=0；
  父节点仍有未完成子节点时不能完成。
- Qt 可创建/编辑/删除节点，修改状态，释放字段锁，并触发单节点或全树严格审视。
- 每次 AI 树更新立即写 SQLite，不等待整轮结束。

### 6. Prompt (prompt.rs)
PromptBuilder: personality + project_instructions + tool_descriptions + session_context
`.with_vector_guidance(...)` 接收 `vector.rs` 的本地检索结果。KnowledgeBase 只索引
`.mooncoding/knowledge/*.md|txt` 与 `agent-memory.jsonl`，采用 256 维哈希向量和余弦相似度，
每轮最多注入 5 个相关块。`memory` tool 可搜索或持久化已验证工程经验，不扫描整个代码库。

## 配置

三层覆盖:
1. 环境变量: MOONCODING_BASE_URL / MOONCODING_MODEL / MOONCODING_API_KEY
2. .mooncoding.toml (当前目录或 ~/.config/mooncoding/)
3. Qt 设置覆盖（URL、model、agent 参数；API key 不由 UI 明文保存）

## 调试

每个模块都可独立调试:
- provider: `cargo test provider` + curl 手动 SSE
- tools: `cargo test tools` 各 tool 独立单元测试
- agent: mock provider + assert AgentEvent 顺序
- config: 单元测试三层覆盖
- tree: CRUD、版本冲突、环检测、人工字段锁、证据门禁
- FFI: C 字符串所有权、树 JSON round-trip、callback 线程切换
- Qt: `TreeModel` 层级、稳定 ID、状态与 ownership roles

## 文件依赖图

```
lib.rs
├── ffi.rs → desktop.rs → session.rs / tree.rs
├── config.rs (无内部依赖)
├── agent.rs
│   ├── provider.rs
│   ├── tools/mod.rs → tools/{bash(verify_command),read,grep,glob,memory,tree,vibe}.rs
│   ├── session.rs
│   ├── tree.rs
│   ├── vector.rs
│   ├── prompt.rs
│   └── stream.rs
