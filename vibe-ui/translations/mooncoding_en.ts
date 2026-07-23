<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE TS>
<TS version="2.1" language="en">
<context>
    <name>MainWindow</name>
    <message><source>对话</source><translation>Chat</translation></message>
    <message><source>项目树</source><translation>Project Tree</translation></message>
    <message><source>历史</source><translation>History</translation></message>
    <message><source>暂无活跃节点</source><translation>No active node</translation></message>
    <message><source>后端加载中</source><translation>Loading backend</translation></message>
    <message><source>0 tokens</source><translation>0 tokens</translation></message>
    <message><source>MoonCoding</source><translation>MoonCoding</translation></message>
    <message><source>主题</source><translation>Theme</translation></message>
    <message><source>切换浅色/深色主题</source><translation>Toggle light/dark theme</translation></message>
    <message><source>设置</source><translation>Settings</translation></message>
    <message><source>对话历史</source><translation>Chat History</translation></message>
    <message><source>新对话</source><translation>New Chat</translation></message>
    <message><source>搜索历史</source><translation>Search history</translation></message>
    <message><source>宽屏下项目树显示在对话右侧，小屏设备请通过此页面查看项目树。</source><translation>The project tree appears beside chat on wide screens. Use this page on narrow screens.</translation></message>
    <message><source>后端不可用</source><translation>Backend unavailable</translation></message>
    <message><source>正在停止 Agent，稍后关闭…</source><translation>Stopping agent, closing shortly…</translation></message>
    <message><source>MoonCoding 设置</source><translation>MoonCoding Settings</translation></message>
    <message><source>API Key 从 .mooncoding.toml 或环境变量读取，不会存储在 UI 中。</source><translation>API Key read from .mooncoding.toml or env, not stored in UI.</translation></message>
    <message><source>Base URL</source><translation>Base URL</translation></message>
    <message><source>模型</source><translation>Model</translation></message>
    <message><source>最大步数</source><translation>Max steps</translation></message>
    <message><source>温度</source><translation>Temperature</translation></message>
    <message><source>语言</source><translation>Language</translation></message>
    <message><source>API Key</source><translation>API Key</translation></message>
    <message><source>设置已保存</source><translation>Settings saved</translation></message>
    <message><source>服务商设置将在 MoonCoding 重启后生效。</source><translation>Provider settings take effect after restarting MoonCoding.</translation></message>
    <message><source>%1 步 · %2 入 / %3 出</source><translation>%1 steps · %2 in / %3 out</translation></message>
    <message><source>新建项目…</source><translation>New Project…</translation></message>
    <message><source>打开项目…</source><translation>Open Project…</translation></message>
    <message><source>暂无最近项目</source><translation>No recent projects</translation></message>
    <message><source>选择新项目的父目录</source><translation>Choose parent directory for new project</translation></message>
    <message><source>新建项目</source><translation>New Project</translation></message>
    <message><source>项目文件夹名称：</source><translation>Project folder name:</translation></message>
    <message><source>创建失败</source><translation>Creation failed</translation></message>
    <message><source>无法创建 %1</source><translation>Cannot create %1</translation></message>
    <message><source>打开项目工作区</source><translation>Open project workspace</translation></message>
    <message><source>Agent 忙碌</source><translation>Agent busy</translation></message>
    <message><source>请先停止当前运行再开始新对话。</source><translation>Stop the current agent before starting a new conversation.</translation></message>
    <message><source>请先停止当前运行再切换项目。</source><translation>Stop the current agent before switching projects.</translation></message>
    <message><source>无效工作区</source><translation>Invalid workspace</translation></message>
    <message><source>不是目录：%1</source><translation>Not a directory: %1</translation></message>
    <message><source>Agent 工作中</source><translation>Agent working</translation></message>
    <message><source>后端就绪</source><translation>Backend ready</translation></message>
    <message><source>已就绪。在聊天中描述目标，我会边推进边构建项目树。你随时可以在项目树面板中修正任何节点。</source><translation>Ready. Describe your goal in chat and I'll build the project tree as I go. Correct any node anytime via the tree panel.</translation></message>
    <message><source>严格审视项目树节点 `%1`。</source><translation>Strict review of project tree node `%1`.</translation></message>
    <message><source>严格审视完整的项目树。</source><translation>Strict review of the full project tree.</translation></message>
    <message><source>活跃节点：%1（%2）</source><translation>Active node: %1 (%2)</translation></message>
    <message><source>活跃节点：%1（%2）· %3</source><translation>Active node: %1 (%2) · %3</translation></message>
    <message><source>暂无活跃节点 —— 让 AI 构建项目树，或选择一个节点继续。</source><translation>No active node — let AI build a project tree, or select a node to continue.</translation></message>
</context>
<context>
    <name>ChatWidget</name>
    <message><source>你</source><translation>You</translation></message>
    <message><source>MoonCoding</source><translation>MoonCoding</translation></message>
    <message><source>思考中…</source><translation>Thinking…</translation></message>
    <message><source>输出中</source><translation>Streaming</translation></message>
    <message><source>输入 %1 · 输出 %2 tokens</source><translation>%1 input · %2 output tokens</translation></message>
    <message><source>运行 %1</source><translation>Running %1</translation></message>
    <message><source>%1 完成 · %2 ms</source><translation>%1 finished · %2 ms</translation></message>
    <message><source>%1 失败（退出码 %2）· %3 ms</source><translation>%1 failed (exit %2) · %3 ms</translation></message>
    <message><source>失败</source><translation>Failed</translation></message>
    <message><source>错误</source><translation>Error</translation></message>
    <message><source>需要处理</source><translation>Action required</translation></message>
    <message><source>已中断</source><translation>Interrupted</translation></message>
    <message><source>Agent 已停止</source><translation>Agent stopped</translation></message>
    <message><source>历史</source><translation>History</translation></message>
</context>
<context>
    <name>InputWidget</name>
    <message><source>发送</source><translation>Send</translation></message>
    <message><source>描述下一步改动、修正某个树节点，或发起一次严格审视…</source><translation>Describe the next change, correct a tree node, or ask for a strict review…</translation></message>
    <message><source>附加上下文文件</source><translation>Attach context files</translation></message>
    <message><source>停止</source><translation>Stop</translation></message>
    <message><source>
显式上下文文件：
- %1</source><translation>
Explicit context files:
- %1</translation></message>
    <message><source>Agent 工作中 · 按停止可中断</source><translation>Agent working · press Stop to interrupt</translation></message>
    <message><source>后端不可用 · 草稿已保留</source><translation>Backend unavailable · draft preserved</translation></message>
    <message><source>Ctrl+Enter 发送 · %1 字%2</source><translation>Ctrl+Enter to send · %1 chars%2</translation></message>
    <message><source> · %1 个文件</source><translation> · %1 file(s)</translation></message>
</context>
<context>
    <name>TreeWidget</name>
    <message><source>项目树工具</source><translation>Tree tools</translation></message>
    <message><source>新增</source><translation>Add</translation></message>
    <message><source>编辑</source><translation>Edit</translation></message>
    <message><source>审视节点</source><translation>Review node</translation></message>
    <message><source>审视全部</source><translation>Review all</translation></message>
    <message><source>刷新</source><translation>Refresh</translation></message>
    <message><source>新增子节点</source><translation>Add child node</translation></message>
    <message><source>编辑节点</source><translation>Edit node</translation></message>
    <message><source>设置状态</source><translation>Set status</translation></message>
    <message><source>待处理</source><translation>Pending</translation></message>
    <message><source>进行中</source><translation>In progress</translation></message>
    <message><source>已完成</source><translation>Completed</translation></message>
    <message><source>失败</source><translation>Failed</translation></message>
    <message><source>需审查</source><translation>Needs review</translation></message>
    <message><source>已阻塞</source><translation>Blocked</translation></message>
    <message><source>已拒绝</source><translation>Rejected</translation></message>
    <message><source>已取消</source><translation>Cancelled</translation></message>
    <message><source>审视此节点</source><translation>Review this node</translation></message>
    <message><source>解除人工字段锁</source><translation>Release human field locks</translation></message>
    <message><source>删除分支</source><translation>Delete branch</translation></message>
    <message><source>通过</source><translation>Passed</translation></message>
    <message><source>确定删除「%1」及其所有子节点？</source><translation>Delete &quot;%1&quot; and all child nodes?</translation></message>
</context>
<context>
    <name>TreeModel</name>
    <message><source>人工: %1</source><translation>Human: %1</translation></message>
    <message><source>AI: %1</source><translation>AI: %1</translation></message>
    <message><source>修订 %1</source><translation>Revision %1</translation></message>
    <message><source>状态</source><translation>Status</translation></message>
    <message><source>工作项</source><translation>Work item</translation></message>
    <message><source>类型</source><translation>Type</translation></message>
    <message><source>优先级</source><translation>Priority</translation></message>
    <message><source>负责人</source><translation>Owner</translation></message>
    <message><source>文件</source><translation>Files</translation></message>
    <message><source>待处理</source><translation>Pending</translation></message>
    <message><source>进行中</source><translation>In progress</translation></message>
    <message><source>已完成</source><translation>Completed</translation></message>
    <message><source>失败</source><translation>Failed</translation></message>
    <message><source>需审查</source><translation>Needs review</translation></message>
    <message><source>已阻塞</source><translation>Blocked</translation></message>
    <message><source>已拒绝</source><translation>Rejected</translation></message>
    <message><source>已取消</source><translation>Cancelled</translation></message>
    <message><source>人工锁定</source><translation>Human locked</translation></message>
    <message><source>人工</source><translation>Human</translation></message>
    <message><source>AI</source><translation>AI</translation></message>
    <message><source>系统</source><translation>System</translation></message>
</context>
<context>
    <name>RustBridge</name>
    <message><source>无法加载 Rust 后端：%1</source><translation>Unable to load Rust backend: %1</translation></message>
    <message><source>Rust 后端 API 不兼容。</source><translation>Rust backend has an incompatible API.</translation></message>
    <message><source>Rust 后端 API 版本 %1 不受支持。</source><translation>Rust backend API version %1 is unsupported.</translation></message>
    <message><source>无法初始化 Rust 后端。</source><translation>Unable to initialize Rust backend.</translation></message>
    <message><source>请先停止 Agent 再切换项目或对话。</source><translation>Stop the agent before switching projects or conversations.</translation></message>
    <message><source>后端未就绪。</source><translation>Backend is not ready.</translation></message>
    <message><source>Agent 正在工作中。</source><translation>The agent is already working.</translation></message>
    <message><source>无法发送消息。</source><translation>Unable to send message.</translation></message>
    <message><source>无法中断 Agent。</source><translation>Unable to interrupt the agent.</translation></message>
    <message><source>请等待 Agent 空闲后再审视节点。</source><translation>The agent must be idle before a node review.</translation></message>
    <message><source>无法审视所选节点。</source><translation>Unable to review the selected node.</translation></message>
    <message><source>请等待 Agent 空闲后再审视项目树。</source><translation>The agent must be idle before a tree review.</translation></message>
    <message><source>无法审视项目树。</source><translation>Unable to review the project tree.</translation></message>
    <message><source>无效的后端事件：%1</source><translation>Invalid backend event: %1</translation></message>
    <message><source>无效的后端树更新：%1</source><translation>Invalid tree update from backend: %1</translation></message>
    <message><source>未知的后端事件类型。</source><translation>Unknown backend event variant.</translation></message>
    <message><source>后端树 API 不可用。</source><translation>Backend tree API is unavailable.</translation></message>
    <message><source>后端返回空响应。</source><translation>Backend returned an empty response.</translation></message>
    <message><source>后端返回格式错误的 JSON。</source><translation>Backend returned malformed JSON.</translation></message>
</context>
</TS>
