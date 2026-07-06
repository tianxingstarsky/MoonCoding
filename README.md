# MoonCoding — Block-Set Vibe Coding CLI

> **Block-level code editing for AI agents**. No more whole-file line-locking.

## What

MoonCoding is a novel vibe-coding protocol and CLI: instead of letting AI edit code files
line-by-line (fragile, error-prone), it splits code into **function-level blocks**.
AI reads only the blocks it needs, edits whole blocks atomically, and the tool
re-assembles source files byte-perfect.

| Traditional vibe coding | MoonCoding |
|---|---|
| AI reads entire file, locks lines | AI reads block skeleton via `overview`, drills into specific blocks via `read` |
| Line edits easily clash, lose context | Whole-block replace, byte-identical guarantee |
| Error on line 42 means re-read whole file | `vibe lookup line 42` → block seq + local_line → `vibe read` that block |
| Lost context = re-read from scratch | Block summaries (`tail.purpose`) persist in index; new AI sessions skim structure instantly |
| O(n²) token growth per conversation | Context pruning keeps history to recent 6 turns; system prompt ~800 tokens |

## Repository

```
MoonCoding/
├── vibe/            # Main CLI: 12 commands (split, assemble, verify, overview, read, peek,
│   ├── src/         #   insert, replace, drop, new, meta, lookup, linemap, deps)
│   ├── test/        #   30+ regression tests (PowerShell suite)
│   └── README.md    #   Full protocol manual
│
└── vibe-test/       # LLM test runner (DeepSeek v4-flash)
    ├── src/         #   db, llm, runner, prompts, session, tools, report
    ├── prompts/     #   system_prompt.md (~800-token agent prompt)
    ├── fixtures/    #   Specs: 01-todo-min, 02-todo-with-css, 03-todo-refactor
    └── runs/        #   (gitignored) JSONL + SQLite + workspace snapshots
```

## Quick start

```bash
# 1. Build the CLI
cd vibe && cargo build --release

# 2. Split an existing Python file into function-level blocks
./target/release/vibe split src/app.py --purpose "REST API entry"

# 3. AI reads the file skeleton (block seqs + summaries + line ranges)
./target/release/vibe overview src/app.py

# 4. AI reads one block's code with line-number prefix
./target/release/vibe read src/app.py 2

# 5. AI replaces a block (stdin JSON, PowerShell pipe)
echo '{"rev":1,"seq":2,"code":"def greet(name):\n    print(f\"hi {name}\")\n","tail":{"summary":"greet(name)","purpose":"says hi"},"purpose_decision":{"unchanged":true}}' |
  ./target/release/vibe replace src/app.py

# 6. Assemble source file from blocks (byte-identical)
./target/release/vibe assemble src/app.py -o src/app.py

# 7. Verify byte-level consistency
./target/release/vibe verify src/app.py
```

## LLM test runner

Let DeepSeek v4-flash drive the vibe CLI autonomously and build real multi-file projects:

```powershell
$env:DEEPSEEK_API_KEY = "your-key"
cd vibe-test

vibe-test list                     # show available specs
vibe-test run 01-todo-min          # AI builds a TodoList webapp from scratch
vibe-test run-all                  # run all 3 specs, inheriting workspaces
vibe-test report                   # view report for latest run
```

The AI successfully built a complete Flask + HTML + JS TodoList demo (3 files, 4~8 blocks)
using ONLY vibe CLI commands — no manual file editing.

## All commands (vibe CLI)

| Command | Purpose |
|---|---|
| `new` | Create empty blockset |
| `split` | Split existing Python file into blocks (tree-sitter AST) |
| `info` | Technical dump: ulid, byte offsets, symbols |
| `overview` | AI-facing file skeleton |
| `peek` | AI-facing one-block narrative |
| `read` | AI-facing code with `NNN:` line-number prefix |
| `meta` | Update top-level purpose only |
| `insert` | Insert new block (stdin JSON) |
| `replace` | Replace whole block (stdin JSON) |
| `drop` | Delete block (preserved in deleted[] history) |
| `assemble` | Concatenate blocks → source file + write line-map.json |
| `verify` | Byte-level invariant + sha256 check |
| `lookup` | Source line → block seq + local_line (error mapping) |
| `linemap` | Dump line-map.json ranges |
| `deps` | Per-block defines/uses/depends_on graph |

## Protocol layers

```
┌── Storage layer (.vibe/<ulid>.vibe/) — #CX program-managed, AI never sees ──┐
│ index.json + blocks.vib.code + line-map.json                                │
│ Fields: ulid, byte_offset, seq, rev, symbols.{defines, uses}                │
└─────────────────────────────────────────────────────────────────────────────┘

                        ↓ program rendering

┌── View layer (AI-facing) — #AI friendly ──────────────────────────────────┐
│ overview: name + [seq] summary + line ranges + rev                          │
│ peek:     tail.purpose (narrative)                                         │
│ read:     {line-number prefix, raw code}                                   │
│ write:    stdin JSON (only #AI fields; #CX auto-filled)                    │
└─────────────────────────────────────────────────────────────────────────────┘

                        ↓ write commands

┌── Cross-block WARNs (P7) ──────────────────────────────────────────────────┐
│ replace/drop → if removed symbol is used by other blocks → WARN            │
│ deps command → full defines/uses/depends_on graph for AI newcomers         │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Design choices

- **Rust** — single binary, no runtime, tree-sitter builtin
- **Tree-sitter** — AST-level block splitting (no regex false positives on string/comment "def")
- **ULID** — globally unique block IDs, stable across sessions
- **Rev** — monotonic version counter prevents stale edits
- **Purpose_decision** — protocol-level forced annotation prevents narrative drift
- **Byte-level invariant** — assemble → verify guarantee: concat(all blocks) == original source
- **Pruning** — context window bounded to 6 recent assistant turns (inspired by opencode)

## Dependencies

```
serde + serde_json    serialization
sha2                 sha256 verification
ulid                 global unique IDs
tree-sitter          Python AST splitting
reqwest              HTTP client (vibe-test)
rusqlite             session DB (vibe-test)
tokio                async runtime (vibe-test)
```

Zero network dependencies at runtime (only compile-time crate downloads).

## Roadmap

| Phase | Status |
|---|---|
| P1 Storage + split + assemble + verify | ✅ |
| P2 View layer: overview / peek / read | ✅ |
| P3 Write commands: insert / replace / drop + rev + remap | ✅ |
| P4 Char-gram TF-IDF purpose-drift WARN | ✅ |
| P5 Tree-sitter AST splitting | ✅ |
| P6 line-map.json + lookup (error→block mapping) | ✅ |
| P7 Cross-block symbol dependency WARN + deps command | ✅ |
| P7.5 LLM test runner (DeepSeek v4-flash) | ✅ |
| P8 Cross-language extension (Rust/TS/Go — plugin slot ready) | TBD |