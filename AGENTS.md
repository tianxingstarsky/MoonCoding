# AGENTS.md — MoonCoding AI Agent Instructions

## 项目定位

MoonCoding 是一个**面向个人的工程化 CLI 交互智能体**。

与 opencode 的本质差异：
- opencode: AI 自主跑, 人可以打断, 但不能"重塑计划树"
- MoonCoding: **人工强驱, 人机协同**——人画项目树, AI 填树; 人标注重点节点, AI 针对性编辑

## 核心价值观

1. **协议安全 > 便利**: 字节一致、rev 防撞、purpose 强制, 这些是底线, 永远不要为了便利而绕过
2. **模块独立调试**: 每个模块必须能独立跑、独立验证, 不依赖全局状态
3. **扩展槽优先**: 新功能 = 加一个 impl / 注册一个 tool, 绝对不能改核心循环
4. **CLI 是核心引擎**: 所有功能先以纯文本 CLI 落地, 之后 UI 只是套壳
5. **精简代码量**: 2500 行 vibe + ~1000 行 vibe-agent 要匹敌 opencode 30 个包的核心能力

## 目录结构

```
MoonCoding/
├── vibe/                    # L1 协议引擎: 区块 CLI (12 命令, Rust, 零 warning)
│   ├── src/                 # format / split / assemble / deps / embed
│   ├── test/suite.ps1       # 30+ 回归测试
│   └── README.md            # 完整协议手册
│
├── vibe-agent/              # L2 交互智能体 (Rust, tokio + async)
│   ├── src/
│   │   ├── main.rs          # CLI: vibe-agent chat / list / resume
│   │   ├── config.rs        # .mooncoding.toml + env 三层合并
│   │   ├── provider.rs      # OpenAI-compatible SSE client (DeepSeek/Groq/Ollama)
│   │   ├── stream.rs        # AgentEvent 枚举 (CLI 消费 → 终端, HTTP → SSE)
│   │   ├── agent.rs         # run_agent() — 主循环 + tool dispatch
│   │   ├── tools/mod.rs     # Tool trait + ToolRegistry
│   │   ├── tools/bash.rs    # shell 子进程
│   │   ├── tools/read.rs    # 读文件
│   │   ├── tools/grep.rs    # 正则搜
│   │   ├── tools/glob.rs    # 通配符找文件
│   │   ├── tools/todowrite.rs  # 树形 TODO (parent_id 层级)
│   │   └── tools/vibe.rs    # vibe CLI wrapper
│   ├── prompts/agent.md     # Agent 人格 system prompt
│   └── ARCHITECTURE.md      # 扩展槽设计文档
│
├── vibe-test/               # 协议验证工具 (batch runner, 回归测试保留)
├── AGENTS.md                # 本文件
├── CONTEXT.md               # 领域词汇表
└── README.md                # 仓库级 readme
```

## 六个扩展槽 (Phase A 就建好, Phase B+ 落地)

操作原则: 加任何新功能都走扩展槽, 严禁直接改 agent.rs 主循环。

| 槽 | 位置 | 用途 | Phase B 示例 |
|---|---|---|---|
| Provider | `provider.rs` trait | 加 Claude/本地模型 | `impl Provider for Anthropic` |
| Tool | `tools/mod.rs` register() | 加新工具 | `registry.register(LspLookupTool)` |
| AgentEvent | `stream.rs` enum | 加新事件类型 | `TreeUpdated { json }` |
| Session | `session.rs` trait | 换存储后端 | `impl SessionStore for Postgres` |
| ProjectTree | `session.rs` Session 字段 | 树形项目管理 | `TreeTool` 接入 |
| Prompt | `prompt.rs` PromptBuilder | 向量/上下文注入 | `.with_vector_context(emb)` |

## 编码规范

- 每个 module 写完立即 `cargo build` 验证, 不攒到最后
- 禁止 unwrap —— 用 anyhow::Result + `?` 传播
- 工具输出截断到 4000 字符 (TOOL_OUTPUT_MAX_CHARS)
- 上下文 pruning: 12 步后保留最近 6 个 assistant 轮次
- LLM 调用: 只走 SSE streaming, 不做非流式

## 当前完成度

| 模块 | 状态 |
|---|---|
| L1 vibe CLI (P1-P7) | ✅ 100% |
| L2 vibe-agent Phase A | ✅ 100% |
| L2 tool: bash | ✅ |
| L2 tool: read | ✅ |
| L2 tool: grep | ✅ |
| L2 tool: glob | ✅ |
| L2 tool: todowrite | ✅ |
| L2 tool: vibe | ✅ |
| L2 agent loop | ✅ |
| L2 session (SQLite) | ✅ |
| L2 config (.mooncoding.toml + env) | ✅ |
| L2 provider (OpenAI-compatible) | ✅ |
| L3 树形项目管理 | Phase B |
| L4 向量工程提示 | Phase C |
| L5 可视化 / 第三方 API | Phase D |
