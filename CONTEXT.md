# CONTEXT.md — MoonCoding Domain Vocabulary

本文档定义 MoonCoding 的领域词汇, 消除歧义。所有 AI agent 在操作此项目前必须阅读。

## 核心概念

### Block (区块)
一个函数/类级别的编辑单元。block 是 vibe 协议的"最小可安全替换对象"。
- block 内包含: 字节正文 + 头索引(CX自动) + 尾索引(AI负责) 
- 不变式: 全块按存储顺序拼接 == 原文件字节
- block 的 identities: ULID(永不变) + seq(自动重编号) + rev(单调递增)

### BlockSet (区块集)
一个源文件的全部 block 集合。存储于 `.vibe/<ulid>.vibe/` 目录:
- `index.json` — 文件索引 + 各块头/尾元数据 + symbols(define/uses)
- `blocks.vib.code` — 各块字节按序拼接
- `line-map.json` — 源文件行号→block seq 的二分查找表

### Fileset (文件集)
即 BlockSet 的别名。`index.json` 中 `fileset: { ulid, name, path, lang, purpose, breakdown, source_sha256 }`

### Rev (单调版本号)
每个 BlockSet 的 `rev` 字段。每次 insert/replace/drop/assemble 自增 1。
AI 必须用最新 rev 操作, 使用旧 rev → `ERR: rev stale`。

### Purpose (顶层功能说明)
`fileset.purpose` — AI 在写命令时必须显式声明是否变更 (`purpose_decision.changed` vs `unchanged`)。强制机制, 不可绕过。

### Breakdown (功能细分)
`fileset.breakdown` — 由程序从各块 `tail.summary` 自动汇总(CX derived)。AI 改块尾索引后自动刷新。

### Tail (尾索引)
每个 block 的尾部元数据, 全由 AI 填写:
- `tail.summary` — 函数签名行 (如 "greet(name: str) -> None")
- `tail.purpose` — 口语化叙事 (如 "函数 greet: 接收 name, 用 f-string 打印问候")

### Symbols (符号表)
每个 block 的 `symbols: { defines, uses }` — CX derived:
- `defines` — tree-sitter 提取的 function/class 名
- `uses` — 块内所有 identifier (去重, 排除关键字、字符串、注释)
- 用于跨块告警: 改名/删块时自动扫所有其它块 uses 有无匹配

### 三档视图
AI 读文件的三种粒度:
1. `overview` — 文件骨架: `[seq] summary` + 行区域 + rev
2. `peek` — 单块叙事: tail.purpose
3. `read` — 单块原码: `NNN:` 行号前缀 (同时是编辑锚)

## 扩展槽概念

六个在 Phase A 就已预留的抽象层:

### Provider 槽
LLM 后端的抽象。`trait Provider` 的 `chat_stream(messages, tools, cb) → (in_tok, out_tok)`。openai-compatible 是唯一内置实现; 加 Claude 只需另一个 impl。

### Tool 槽
`trait Tool: { name, description, parameters, execute }`。`ToolRegistry::register()` 注册后自动出现在 LLM 工具列表中。新工具 = 新 impl, 不改 agent 循环。

### AgentEvent 槽
`enum AgentEvent` — agent 运行中产生的所有事件。CLI 消费事件渲染到终端; 未来 HTTP 消费事件转 SSE; 同一套事件, 两个消费端。

### Session 槽
`trait SessionStore: { load, save, list, latest }`。当前是 SQLite 实现; 换 PostgreSQL 只换 impl。

### ProjectTree 槽
`Session.project_tree: Option<ProjectTree>` — 持久树形项目管理。节点记录状态、分支类型、关联文件、
验证证据、修改者与字段级人工锁。AI 不得覆盖人工字段；没有成功证据不得自行标记完成。

### Prompt 槽
`PromptBuilder` — agent 系统提示分层构造器:
- `personality` (agent.md 人格基板)
- `project_instructions` (AGENTS.md 扫描)
- `tool_descriptions` (ToolRegistry 自动生成)
- `session_context` (剪枝标记 + TODO 状态)
- `vector_guidance`：从 `.mooncoding/knowledge` 与已验证 memory 中只检索当前任务相关片段

## 两类角色

### #AI (AI 负责)
AI 必须填写或显式声明的字段: `purpose`, `tail.{purpose, summary}`, `name`, `lang`, `path`

### #CX (程序自动化)
程序管理的字段, AI 永不见: `ulid`, `seq`, `byte_offset`, `rev`, `breakdown`, `source_sha256`, `line_map`, `symbols`

## Agent 交互模型

```
用户在 Qt 桌面输入任务
    ↓
Agent 先通过 tree tool 画持久项目树
    ↓
Agent 扫描项目 (read/grep/glob)
    ↓
Agent 用 vibe 协议编辑代码 (vibe tool: insert/replace/drop/assemble/verify)
    ↓
错误回映 (lookup → 定位 block → 替换)
    ↓
跨块依赖警告自动触发 (rename 后提示相关块)
    ↓
上下文剪枝: 12 步后保留最近 6 个 assistant 轮次
    ↓
用户点击检查/强驱 (修改树节点/状态, 创建分支, 单节点或全树审视)
```

## 与 opencode 的关键差异

| | opencode | MoonCoding |
|---|---|---|
| 任务管理 | 扁平 TODO | 持久树 + 字段级人工权威 + 验证证据 |
| 编辑模型 | 整文件行锁 | 区块级整块替换 |
| 用户角色 | 可用打断, 不能重塑 | 人工强驱: 画树, 标注, 修改 |
| 权限 | Deferred ask 完整版 | v1 沙箱白名单, v2 加人工审批 |
| 证明机制 | 无 | rev + purpose_decision 双重强制 |
| 错误定位 | 需要 grep 全文 | line-map + lookup 即时映射 |
