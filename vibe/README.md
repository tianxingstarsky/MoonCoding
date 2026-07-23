# vibe — 函数级区块集 Vibe Coding CLI

> **核心思想**：传统 vibe coding 把整文件丢给 AI 锁行改动，易错、易全项目重来。
> vibe 反过来——把代码按函数/类细粒度切到多个 **区块(block)**，每个区块带说明。
> AI 只读必要区块 + 带行号前缀的精确视图；改代码走"整块替换"，程序自动重编号、自动维护版本号。
> 区块是真相源，源文件可随时重建，字节级一致。

## 为什么区块化

| 痛点 | 区块怎么做 |
|---|---|
| AI 看全文浪费 token、易错 | 三档视图：overview(粗取) / peek(精准) / read(原码+行号) |
| 锁行改易撞 | 整块替换，零歧义；区块是唯一编辑单元 |
| 上下文爆掉/新 AI 上手 | 只读 overview 即得文件骨架，不必读全文 |
| 删改后旧记忆失效 | rev 单调递增；用旧 rev 操作直接 `ERR: rev stale` |
| 文件被乱删 | drop 只删块+留档 `deleted[]`；区块集可重新 assemble |
| 顶层"功能说明"漂移没人发现 | 写命令强制带 `purpose_decision`；assemble 跑 TF-IDF 兜底 WARN |

---

## 快速上手

```bash
# 拆一个现有 Python / Rust / C++ 文件 → 生成区块集（按扩展名识别）
vibe split src/app.py --purpose "REST API 入口"

# AI 视角：粗取一份文件骨架
vibe overview src/app.py
#   file: app.py
#   purpose: REST API 入口
#   rev: 1
#   (assembled: 未 assemble; 行号以最近 assemble 后为准)
#     [ 1] import os                          lines 1-3
#     [ 2] greet(name: str) -> None          lines 5-7
#     [ 4] main(argv) -> int                 lines 9-15

# AI 想看某块的代码：read，带 `NNN: ` 行号前缀，可直接做修改锚
vibe read src/app.py 4
#   [4] rev=1 lines 9-15
#   009: def main(argv):
#   010:     greet("world")
#   011:     return 0

# AI 改一块：从 stdin 喂 JSON（只填 #AI 字段，#CX 由程序补）
echo '{"rev":1,"seq":4,"code":"def main(argv):\n    greet(\"earth\")\n    return 0\n","tail":{"summary":"main(argv)","purpose":"入口调用 greet"},"purpose_decision":{"changed":"REST API 入口"}}' | vibe replace src/app.py
#   {"ok":true,"new_rev":2,"seq":4,"binding":"01HXY..."}

# 产生最终源文件
vibe assemble src/app.py -o src/app.py
vibe verify src/app.py                # 字节级一致性
```

---

## 存储层布局（#CX 程序管理，AI 不直接读）

```
<project_root>/
└── .vibe/<fileset_ulid>.vibe/
    ├── index.json          # 集合索引 + 各区块头/尾元数据
    └── blocks.vib.code     # 各区块代码字节连续存储（保字节一致）
```

- `blocks.vib.code` = 各区块 `code` 字段按顺序拼接，`index.json` 里记录每块的 `byte_offset/byte_length`。
- 不变式：`concat(blocks[*].data) == blocks.vib.code`，assemble 直接拷这段字节。
- `index.json` 顶层结构（schema 见 `src/format.rs`）：

```jsonc
{
  "rev": 7,                                  // 单调递增，AI 用它对账
  "fileset": {
    "ulid":   "01HXY...",                     // #CX 全局唯一
    "name":   "app.py",                       // #AI
    "path":   "src/app.py",                   // #AI  POSIX 相对路径
    "lang":   "python",                       // #AI
    "purpose":"REST API 入口",                // #AI（变更时必须更新）
    "breakdown": ["greet(name)->None","main(argv)->int"],  // #CX derived: 汇总各块 tail.summary
    "source_sha256": "<hex|空>"               // #CX  最近一次 assemble 写出源文件 hash
  },
  "blocks": [
    {
      "ulid":        "01HXY...",              // #CX  绑定ID，永不变
      "seq":         2,                      // #CX  物理顺序（insert/drop 后自动重编号）
      "byte_offset": 47,                     // #CX  在 blocks.vib.code 中偏移
      "byte_length": 51,                     // #CX
      // 头索引的"关联行数"不持久化：每次读取时程序按字节累加实时算
      "tail": {
        "purpose": "函数 greet(name): 用 f-string 打印问候",   // #AI  口语化叙事
        "summary": "greet(name: str) -> None"                   // #AI  带函数名/签名的简介
      }
    }
  ],
  "deleted": [                              // drop 后留档，可回滚
    { "ulid":"...","seq_was":3,"tail":{...},"deleted_at_rev":7,"byte_length":40 }
  ]
}
```

### #AI / #CX 字段标注约定

- `#AI`：AI 负责编辑或必须显式声明的字段（`purpose`、`tail.purpose`、`tail.summary`、`path`、`name`、`lang`）。
- `#CX`：程序自动化管理的字段（`ulid`、`seq`、`byte_offset/length`、`breakdown`、`source_sha256`、`rev`、`关联行数`）。
- AI 不必读 `index.json`，全部交互通过 CLI 视图层完成。

---

## AI 视图层（三档读取）

| 命令 | 作用 | 输出样本 |
|---|---|---|
| `vibe overview <path>` | 整文件骨架：purpose + 各块 `[seq] summary` + 行区域 + 当前 rev | 顶部带 `rev:` 与 `assembled:` 状态 |
| `vibe peek <path> <seq>` | 单块的口语化叙事（`tail.purpose`） | 行区域 + purpose |
| `vibe read <path> <seq>` | 单块带行号前缀的原代码 | `005: def greet(name):` ... |

- AI 看到的只有 **`seq`（顺序号）+ `rev`**，看不到 ulid/byte_offset。
- 行号前缀同时是修改锚：AI 直接引用行号即可。

---

## 写入协议（AI 走 stdin JSON）

所有写命令必须带 **`purpose_decision`** 字段：
- `{"changed":"新的顶层 purpose"}` —— 显式声明 fileset purpose 已变
- `{"unchanged":true}` —— 显式声明不变

**这是协议级强制**，避免"功能说明"漂移在累计改动中无人发现。

### `vibe insert <path>  < stdin`
```json
{
  "rev": 7,
  "after": 2,                          // 插到 seq=2 之后；after=0 插到最前
  "code": "def mul(a, b):\n    return a*b\n\n",  // 该块字节
  "tail": {
    "summary": "mul(a: int, b: int) -> int",
    "purpose": "两数相乘，返回 a*b"
  },
  "purpose_decision": { "changed": "新增乘法 demo" }
}
```
回执：`{"ok":true,"new_rev":8,"new_seq":3,"binding":"01HX..."}`

### `vibe replace <path> < stdin`
```json
{
  "rev": 7, "seq": 2,
  "code": "def greet(name):\n    print(f\"hi {name}\")\n",
  "tail": { "summary":"greet(name)","purpose":"改为 hi 问候" },
  "purpose_decision": { "unchanged": true }
}
```
回执：`{"ok":true,"new_rev":8,"seq":2,"binding":"01HX..."}`（绑定ID不变）

### `vibe drop <path> < stdin`
```json
{ "rev": 7, "seq": 3, "purpose_decision": { "unchanged": true } }
```
- 块移到 `deleted[]`，`assemble` 时跳过；后续想回滚就重建块。
- `seq` 自动重编号；写命令回执的 stderr 里有 `remap:` 表（调试用，AI 不必解析）。

### `vibe meta <path> --purpose "..."`
单独更新顶层 `fileset.purpose` 而不动块。

### rev 校验
- `rev` 在每次写操作自增。AI 操作时必须带最新 `rev`，否则 `ERR: rev stale`，强制重跑 `overview`。
- 这把"AI 记忆过期"变成可预测的硬错误，而非默默错位。

---

## 拼装与一致性

```bash
vibe assemble src/app.py -o src/app.py      # 拼接写出 + 回写 source_sha256 + rev++ + 写 line-map.json
vibe verify   src/app.py                    # 不变式校验 + 与磁盘源文件字节比对
```

- `assemble` 后 `blocks.vib.code == 源文件字节`，sha256 永久记录在 `index.json`。
- `assemble` 同步写出 `line-map.json`：源文件行号 → 区块 seq 的紧凑 ranges，二分查找回映。
- `verify` 检查两件事：(a) 内部不变式；(b) 若 `src/app.py` 存在则与之按字节比对。
- 测试覆盖：`simple.py` 和 `sample.py`（含嵌套 class 与装饰器）各 round-trip 字节一致、哈希匹配。

## 行号映射（错误回映）

```bash
vibe assemble src/app.py            # 先 assemble, 才有 line-map
vibe lookup  src/app.py 12          # 源文件第 12 行 -> seq + local_line + summary
vibe linemap src/app.py             # dump 整张 line-map 调试用
```

`line-map.json` 顶层结构（保字节一致性的派生产物，assemble 时同步刷新）：
```json
{
  "rev": 7,
  "source_sha256": "<hex>",
  "line_count": 21,
  "ranges": [
    {"seq":1,"from":1,"to":5},
    {"seq":2,"from":6,"to":9},
    {"seq":3,"from":10,"to":13},
    {"seq":4,"from":14,"to":21}
  ]
}
```

**用法场景**：
- LSP 报 `app.py:12:UndefinedName` → `vibe lookup src/app.py 12` → `seq=3 local_line=3 summary="def sub(...)"`
- 运行时栈 `File "app.py", line 18` → `vibe lookup src/app.py 18` → `seq=4 local_line=5`
- AI 直接 `vibe peek src/app.py 3` 看口语化叙事 + `vibe read src/app.py 3` 看带行号原码改

不会再"读完文件才知道是哪一行出错"。lookup 输出还顺手给操作 hint：
```
[line 12] -> seq=3  local_line=3
  summary: def sub(...)
  hint: vibe peek src/app.py 3  | vibe read src/app.py 3
```

注意：lookup 不在 `assemble` 之前生效（没有 source 行号），会报 `line-map missing`。

---

## 漂移兜底（assemble 前自动跑）

`vibe assemble` 与每个写命令末尾都会跑：
- `embed::check_drift(fileset.purpose, breakdown)` — 用 **char-gram TF-IDF 余弦相似度**（零依赖、零模型、纯本地）比对顶层叙事与各块汇总简介。
- 阈值 `0.6`。低于阈值 → `WARN: purpose drift cos=... < 0.6; AI 复核`，**不阻断**，只提示。
- 这是一道兜底，**抓大头漂移**（AI 偷懒没改 purpose）；不负责捕捉同义改写。

## 跨块符号依赖告警（P7）

每个块保持源语言的合法字节片段，所以 split / insert / replace 时程序按 fileset.lang
自动使用对应 tree-sitter grammar 提取符号：

- `defines`: AST 里所有 `function_definition` / `class_definition` 的 `name` 字段。
- `uses`: 该块字节里的所有 identifier（已去重、排除语言关键字、字符串字面量与注释里的假标识符不算）。

`defines` 与 `uses` 都作为 #CX derived 写入 `index.json` 的 `Block.symbols`。AI 不直接见这两行——程序用它们做以下自动告警。

### `replace` 后告警（不阻断）

如果替换后块里 `defines` 变了，进一步比对文件级 defines 集合："被移除/改名"的符号如果被其它块的 `uses` 引用 → WARN：

```
WARN: cross-block dep impact (replace): block seq=2 changed ->
  seq=4 "def main(argv: List[str]) -> int:" uses now-removed symbol(s): add
  -> check those blocks; use `vibe peek <path> <seq>` to review.
```

### `drop` 后告警

被删块里"独占"的 define（即离开它整个文件集就不再有人定义）如果被剩余块引用 → WARN：

```
WARN: cross-block dep impact (drop):
  seq=3 "def caller()" uses now-removed symbol(s): add
  seq=4 "def main(...)" uses now-removed symbol(s): add
```

### `vibe deps <path>` — 完整依赖图

```
file: src/simple.py
purpose: simple demo for deps
rev: 1
blocks: 4
fileset_defines: add, main, sub

seq=2  def add(a: int, b: int) -> int:
  defines: add

seq=4  def main(argv: List[str]) -> int:
  defines: main
  uses_fileset: add, sub
  depends_on  : seqs [2, 3]
```

新 AI 上手时跑一次 `vibe deps` 就能看明白：文件里有哪些手机号函数、谁依赖谁，可立即定位"改 add 必动 main"。

### 取舍

**这是 best-effort 启发式**：
- `uses` 用 identifier 抽取，会被同名局部变量、模块属性访问(`obj.greet`) 等"非函数" 引用误触发，所以**只 WARN 不阻断**；AI 一眼就能判断假阳与否。
- 没跨文件符号解析（vibe 是单文件级区块集协议）；跨文件由未来 P9 符号 注册表解决。
- 关键 POS：完全不引入重型 name-resolution，AI token 几乎没消耗（一次 `deps` 命令拿到全图）。

---

## AST 拆分边界规则（写进协议，由 tree-sitter 实现）

1. **实体首行开始切**：root 节点 named_children 中 `function_definition` / `class_definition` / `decorated_definition` 的 `start_byte` 即为新块起点。缩进的 `def`/`class` 是嵌套节点，归到父块。
2. **装饰器链 + 被装饰 def/class 同块**：tree-sitter 把 `@deco1` `@deco2` `def f()` 包装到同一个 `decorated_definition` 节点，自然合成 1 块。
3. **函数 docstring 归其函数块**：紧接 `def` 的三引号说明与函数体同块。
4. **模块 docstring + imports + 全局 = `module_header` 块**（seq=1，即第一个 def 之前的所有 named_children 段）。
5. **首次 split 时 `tail.purpose` 留空**（强制 AI 后续 review/replace 时填写），`tail.summary` 取该块第一非空行（即签名行）。
6. **跨平台路径**：内部统一 POSIX 相对路径（Windows 反斜杠自动转正），assemble 输出时按 OS 转回。
7. **行区域**：`overview`/`read` 显示的行号基于"按当前 blocks 拼接假设"，顶部标注 `(assembled: ...)`，未 assemble 时以"现组装后为准"。
8. **空块集允许**：`vibe new` 建立零块区块集，AI 之后用 `insert` 逐步填块。
9. **字符串/注释里假 def 不会误拆**：tree-sitter 走 AST，不被字符串字面量、行内注释里的 `def ...()` 欺骗（这是它取代行级检测的最大用途）。
10. **语法错误兜底**：tree-sitter 对不完整 / 错误语法会尽力局部解析返回成功，仍可生成块且保字节一致。

---

## 完整命令一览

| 命令 | 用途 | 谁用 |
|---|---|---|
| `vibe new` | 建空区块集 | AI 创建新文件 |
| `vibe split` | 拆现有文件到区块集 | 一次性导入 |
| `vibe info` | 技术结构 dump（带 ulid/byte 信息） | 调试 |
| `vibe overview` | AI 视角文件骨架 | AI |
| `vibe peek` | AI 视角单块叙事 | AI |
| `vibe read` | AI 视角单块带行号原码 | AI |
| `vibe meta` | 仅更新顶层 purpose | AI |
| `vibe insert` | 插入新块 | AI |
| `vibe replace` | 整块替换 | AI |
| `vibe drop` | 删除块（留档） | AI |
| `vibe assemble` | 拼接写出源文件 + 写 line-map.json | AI/CI |
| `vibe verify` | 字节级一致性校验 | CI/AI |
| `vibe lookup` | 源文件行号 → 区块 seq + local_line（错误回映） | LSP 诊断/运行时错误 |
| `vibe linemap` | dump line-map.json 内容（调试） | 调试 |
| `vibe deps` | dump 完整依赖图 defines/uses/depends_on | 新 AI 上手 |

---

## 依赖

`Cargo.toml` 共六个 crate：
- `serde` / `serde_json` — 序列化 `index.json` / `line-map.json`
- `sha2` — assemble 后字节校验
- `ulid` — 全局唯一绑定ID
- `tree-sitter-{python,rust,cpp}` — Python / Rust / C++ AST 拆分（不使用行级正则）

零隐式网络依赖（仅 crate 编译期拉取源）。后续语言仍通过新增 `tree-sitter-<lang>` grammar 扩展。

---

## 路线图

| 阶段 | 状态 |
|---|---|
| P1 存储层 + split + assemble + verify（字节级 roundtrip） | ✅ |
| P2 视图层 overview/peek/read | ✅ |
| P3 写命令 insert/replace/drop + rev 校验 + 重编号 | ✅ |
| P4 TF-IDF 兜底 + WARN（零模型） | ✅ |
| P6 行号映射表 `line-map.json` 供 LSP/运行时错误直接映射到区块 | ✅ |
| P5 tree-sitter 替换 `src/split.rs` 行级检测 | ✅ |
| P7 跨块符号依赖告警（改 B 签名时通知 A） | ✅ |
| P8 跨语言扩展（Rust + C++） | 已实现，待环境回归 |
| P8 后续语言（TS/Go） | 待做 |

---

## 一句话协议

> 整块替换、顺序号操作、rev 校验、字节级一致。
> AI 只看骨架与必要块，绝不读全文、绝不锁行改。