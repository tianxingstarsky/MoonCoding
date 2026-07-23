mod assemble;
mod deps;
mod embed;
mod format;
mod split;

use format::{new_ulid, BlockSet, Fileset, Index, Block, Tail, DeletedBlock, LineMap, sha256_hex};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 { usage(); exit(2); }
    match args[1].as_str() {
        "new"       => cmd_new(&args[2..]),
        "split"     => cmd_split(&args[2..]),
        "info"      => cmd_info(&args[2..]),
        "overview"  => cmd_overview(&args[2..]),
        "peek"      => cmd_peek(&args[2..]),
        "read"      => cmd_read(&args[2..]),
        "insert"    => cmd_insert(&args[2..]),
        "replace"   => cmd_replace(&args[2..]),
        "drop"      => cmd_drop(&args[2..]),
        "meta"      => cmd_meta(&args[2..]),
        "assemble"  => cmd_assemble(&args[2..]),
        "verify"    => cmd_verify(&args[2..]),
        "lookup"    => cmd_lookup(&args[2..]),
        "linemap"   => cmd_linemap(&args[2..]),
        "deps"      => cmd_deps(&args[2..]),
        _ => { usage(); exit(2); }
    }
}

fn usage() {
    eprintln!("vibe (block-set protocol) - commands:");
    eprintln!("  vibe new <path> --name <n> --lang <l> --purpose <p>");
    eprintln!("  vibe split <source> [--lang python|rust|cpp] [--purpose <p>]");
    eprintln!("  vibe info <path|ulid>                          technical structural dump (#CX view)");
    eprintln!("  vibe overview <path>                          AI-facing file summary");
    eprintln!("  vibe peek <path> <seq>                         AI-facing one-block narrative (tail.purpose)");
    eprintln!("  vibe read <path> <seq>                         AI-facing code with line-number prefix");
    eprintln!("  vibe meta <path> --purpose <p>                 update top-level purpose only");
    eprintln!("  vibe insert <path>  < stdin JSON               insert new block");
    eprintln!("  vibe replace <path> < stdin JSON               replace whole block");
    eprintln!("  vibe drop <path>   < stdin JSON                delete block (kept in deleted[] history)");
    eprintln!("  vibe assemble <path> [-o out]                  concatenate blocks -> source file");
    eprintln!("  vibe verify <path> [original.py]               byte-level invariant + sha256 check");
    eprintln!("  vibe lookup <path> <line>                       locate src line -> block seq + local_line");
    eprintln!("  vibe linemap <path>                            dump line-map.json for debugging");
    eprintln!("  vibe deps <path>                                dump per-block defines/uses/depends_on graph");
    eprintln!("stdin JSON schema:");
    eprintln!("  insert : {{rev, after, code, tail:{{summary,purpose}}, purpose_decision:{{changed|unchanged}}}}");
    eprintln!("  replace: {{rev, seq, code, tail:{{summary,purpose}}, purpose_decision:{{changed|unchanged}}}}");
    eprintln!("  drop   : {{rev, seq, purpose_decision:{{changed|unchanged}}}}");
    eprintln!("  purpose_decision: {{\"changed\":\"新顶层说明\"}} | {{\"unchanged\":true}}");
}

fn die(msg: impl AsRef<str>) -> ! {
    eprintln!("error: {}", msg.as_ref());
    exit(1);
}

fn root() -> PathBuf { PathBuf::from(".") }

/// Windows 反斜杠 -> POSIX 斜杠, 内部统一用相对 POSIX 路径
fn to_posix(p: &str) -> String { p.replace('\\', "/") }

fn workspace_path(input: &str) -> Result<PathBuf, String> {
    let portable = input.trim().replace('\\', "/");
    if portable.is_empty() {
        return Err("path must not be empty".to_string());
    }
    let path = Path::new(&portable);
    if path.is_absolute() {
        return Err(format!("path must be workspace-relative: {input}"));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("path escapes workspace: {input}"));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("path must identify a file".to_string());
    }
    Ok(normalized)
}

/// CLI: 支持 <path> 或 <ulid>; 优先按 path 在 .vibe 索引里查。
/// 找不到时：仅当参数像 ULID 才按 ulid 加载，否则给出可执行的 split/new 提示。
fn bs_load(arg: &str) -> BlockSet {
    let posix = to_posix(arg);
    match BlockSet::load_by_path(&root(), &posix) {
        Ok(Some(bs)) => return bs,
        Ok(None) => {}
        Err(e) => die(format!("load by path `{posix}`: {}", io_en(&e))),
    }
    if looks_like_ulid(arg) {
        return BlockSet::load(&root(), arg).unwrap_or_else(|e| {
            die(format!("no blockset for ulid `{arg}`: {}", io_en(&e)))
        });
    }
    die(format!(
        "no blockset for path `{posix}`.\n\
         Create one first, then retry:\n\
           vibe split {posix} --purpose \"<why this file exists>\"\n\
           vibe new {posix} --name <n> --lang <python|rust|cpp> --purpose \"<why>\""
    ))
}

fn looks_like_ulid(s: &str) -> bool {
    let s = s.trim();
    s.len() == 26
        && s.chars()
            .all(|c| matches!(c, '0'..='9' | 'A'..='Z' | 'a'..='z'))
}

/// Prefer stable English io errors so tool output is not locale-garbled in UI/logs.
fn io_en(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(code) => format!("{} (os error {code})", e.kind()),
        None => format!("{}", e.kind()),
    }
}

/// 解析 stdin JSON (整个 body 当 JSON)
fn read_stdin_json() -> serde_json::Value {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap_or_else(|e| die(format!("stdin: {e}")));
    if s.trim().is_empty() { die("stdin empty"); }
    serde_json::from_str(&s).unwrap_or_else(|e| die(format!("parse JSON: {e}")))
}

fn v_str(v: &serde_json::Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn v_u64(v: &serde_json::Value, k: &str) -> u64 {
    v.get(k).and_then(|x| x.as_u64()).unwrap_or_else(|| die(format!("{} missing", k)))
}
fn v_tail(v: &serde_json::Value) -> Tail {
    let t = v.get("tail").unwrap_or_else(|| die("tail missing"));
    Tail { purpose: v_str(t, "purpose"), summary: v_str(t, "summary") }
}
/// 返回 (changed_purpose: Option<String>, unchanged_flag: bool)
fn v_purpose_decision(v: &serde_json::Value) -> (Option<String>, bool) {
    let pd = v.get("purpose_decision").unwrap_or_else(|| die("purpose_decision missing"));
    if let Some(u) = pd.get("unchanged").and_then(|x| x.as_bool()) {
        if u { return (None, true); }
    }
    if let Some(c) = pd.get("changed").and_then(|x| x.as_str()) {
        return (Some(c.to_string()), false);
    }
    die("purpose_decision must be {\"unchanged\":true} or {\"changed\":\"...\"}")
}

fn ensure_rev(bs: &BlockSet, expected_rev: u64) {
    if bs.index.rev != expected_rev {
        die(format!("rev stale: expected {} but current is {} (re-run overview)", expected_rev, bs.index.rev));
    }
}

fn apply_purpose_decision(bs: &mut BlockSet, changed: Option<String>) {
    if let Some(p) = changed {
        bs.index.fileset.purpose = p;
    }
}

fn embed_check(bs: &BlockSet) -> Option<(f64, String)> {
    let (drift, sim) = embed::check_drift(&bs.index.fileset.purpose, &bs.index.fileset.breakdown);
    if drift { Some((sim, format!("WARN: purpose drift cos={:.3} < {}; 顶层 'purpose' 与 'breakdown' 严重偏离, AI 复核", sim, embed::THRESHOLD))) } else { None }
}

// ========== commands ==========

fn cmd_new(args: &[String]) {
    let mut path = String::new(); let mut name = String::new(); let mut lang = String::new(); let mut purpose = String::new();
    let mut i = 0; while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                name = args.get(i + 1).cloned().unwrap_or_else(|| die("new: --name requires a value"));
                i += 2;
            }
            "--lang" => {
                lang = args.get(i + 1).cloned().unwrap_or_else(|| die("new: --lang requires a value"));
                i += 2;
            }
            "--purpose" => {
                purpose = args.get(i + 1).cloned().unwrap_or_else(|| die("new: --purpose requires a value"));
                i += 2;
            }
            _ if path.is_empty() => { path = args[i].clone(); i += 1; }
            _ => die("unknown arg"),
        }
    }
    if path.is_empty() || name.trim().is_empty() || lang.trim().is_empty() || purpose.trim().is_empty() {
        die("new: <path> --name --lang --purpose all required and non-empty");
    }
    let normalized_path = workspace_path(&path).unwrap_or_else(|error| die(error));
    let posix = to_posix(&normalized_path.to_string_lossy());
    if BlockSet::load_by_path(&root(), &posix).unwrap_or(None).is_some() { die(format!("fileset for {} already exists", posix)); }
    let ulid = new_ulid();
    let dir = BlockSet::dir_of(&root(), &ulid);
    fs::create_dir_all(&dir).unwrap_or_else(|e| die(format!("mkdir: {e}")));
    let fileset = Fileset { ulid: ulid.clone(), name, path: posix, lang, purpose,
        breakdown: Vec::new(), source_sha256: String::new() };
    let index = Index { rev: 1, fileset, blocks: Vec::new(), deleted: Vec::new() };
    let bs = BlockSet { root: root(), index, code: Vec::new() };
    bs.save().unwrap_or_else(|e| die(format!("save: {e}")));
    println!("{{\"ok\":true,\"rev\":{},\"ulid\":\"{}\"}}", bs.index.rev, ulid);
}

fn cmd_split(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let src_path = workspace_path(&args[0]).unwrap_or_else(|error| die(error));
    let mut purpose = String::new();
    let mut explicit_language: Option<String> = None;
    let mut i = 1; while i < args.len() {
        match args[i].as_str() {
            "--purpose" => {
                let Some(value) = args.get(i + 1) else { die("split: --purpose requires a value"); };
                purpose = value.clone();
                i += 2;
            }
            "--lang" => {
                let Some(value) = args.get(i + 1) else { die("split: --lang requires a value"); };
                explicit_language = Some(value.clone());
                i += 2;
            }
            _ => die("split: unknown arg"),
        }
    }
    let src = fs::read(&src_path).unwrap_or_else(|e| die(format!("read: {e}")));
    if purpose.trim().is_empty() {
        die("split: non-empty --purpose is required");
    }
    let posix = to_posix(&src_path.to_string_lossy());
    let name = src_path
        .file_name()
        .unwrap_or_else(|| die("split: source path has no file name"))
        .to_string_lossy()
        .to_string();
    if BlockSet::load_by_path(&root(), &posix).unwrap_or(None).is_some() {
        die(format!("fileset for {} already exists; drop it first", posix));
    }
    let language = split::SourceLanguage::detect(&src_path, explicit_language.as_deref())
        .unwrap_or_else(|error| die(error));
    let bs = split::split_source(&src, &root(), &posix, &name, &purpose, language)
        .unwrap_or_else(|error| die(error));
    bs.save().unwrap_or_else(|e| die(format!("save: {e}")));
    println!("split: {} -> .vibe/{}.vibe/ ({} blocks, rev {})",
        posix, bs.index.fileset.ulid, bs.index.blocks.len(), bs.index.rev);
}

fn cmd_info(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    bs.recount_lines();
    let f = &bs.index.fileset;
    println!("ulid   : {}", f.ulid);
    println!("name   : {}", f.name);
    println!("path   : {}", f.path);
    println!("lang   : {}", f.lang);
    println!("purpose: {}", f.purpose);
    println!("breakdown (#CX derived):");
    for s in &f.breakdown { println!("  - {}", s); }
    println!("sha256 : {} {}", if f.source_sha256.is_empty() {"(unassembled)"} else {""}, f.source_sha256);
    println!("rev    : {}", bs.index.rev);
    println!("blocks : {}", bs.index.blocks.len());
    for b in &bs.index.blocks {
        println!("  seq={:>2} ulid={} bytes=[{}..{}] lines=[{}..{}]  summary=\"{}\"",
            b.seq, b.ulid, b.byte_offset, b.byte_offset + b.byte_length,
            b.line_start, b.line_end, b.tail.summary);
    }
    if !bs.index.deleted.is_empty() {
        println!("deleted (history):");
        for d in &bs.index.deleted {
            println!("  seq_was={} ulid={} bytes_len={} summary=\"{}\"  deleted_at_rev={}",
                d.seq_was, d.ulid, d.byte_length, d.tail.summary, d.deleted_at_rev);
        }
    }
}

fn cmd_overview(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    bs.recount_lines();
    let f = &bs.index.fileset;
    println!("file: {}", f.path);
    println!("name: {}", f.name);
    println!("purpose: {}", f.purpose);
    println!("rev: {}", bs.index.rev);
    println!("(assembled: {}; 行号以最近 assemble 后为准)", if f.source_sha256.is_empty() {"未 assemble"} else {"已 assemble"});
    if bs.index.blocks.is_empty() {
        println!("(no blocks yet)");
        return;
    }
    for b in &bs.index.blocks {
        println!("  [{:>2}] {}   lines {}-{}", b.seq, b.tail.summary, b.line_start, b.line_end);
    }
}

fn cmd_peek(args: &[String]) {
    if args.len() < 2 { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    bs.recount_lines();
    let seq: usize = args[1].parse().unwrap_or_else(|e| die(format!("seq: {e}")));
    let b = bs.index.blocks.iter().find(|b| b.seq == seq).unwrap_or_else(|| die(format!("seq {} not found", seq)));
    println!("[{}] {}  (lines {}-{})", b.seq, b.tail.summary, b.line_start, b.line_end);
    println!("purpose: {}", if b.tail.purpose.is_empty() {"(未填写)"} else {&b.tail.purpose});
}

fn cmd_read(args: &[String]) {
    if args.len() < 2 { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    bs.recount_lines();
    let seq: usize = args[1].parse().unwrap_or_else(|e| die(format!("seq: {e}")));
    let b_idx = bs.index.blocks.iter().position(|b| b.seq == seq).unwrap_or_else(|| die(format!("seq {} not found", seq)));
    let b = &bs.index.blocks[b_idx];
    let data = bs.block_bytes(b).to_vec();
    let text = String::from_utf8_lossy(&data);
    println!("[{}] rev={} lines {}-{}", b.seq, bs.index.rev, b.line_start, b.line_end);
    // split('\n') 会在末尾多出一个空元素; 用块行数实际值限制显示
    let want_lines = b.line_end.saturating_sub(b.line_start) + 1;
    let mut ln = b.line_start;
    let mut shown = 0usize;
    for raw_line in text.split('\n') {
        if shown >= want_lines { break; }
        let line = raw_line.trim_end_matches('\r');
        println!("{:03}: {}", ln, line);
        ln += 1;
        shown += 1;
    }
}

fn cmd_meta(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    let mut i = 1; let mut new_purpose: Option<String> = None;
    while i < args.len() {
        match args[i].as_str() {
            "--purpose" => {
                new_purpose = Some(
                    args.get(i + 1)
                        .cloned()
                        .unwrap_or_else(|| die("meta: --purpose requires a value")),
                );
                i += 2;
            }
            _ => die("meta: only --purpose supported"),
        }
    }
    if let Some(p) = new_purpose {
        if p.trim().is_empty() {
            die("meta: --purpose must not be empty");
        }
        bs.index.fileset.purpose = p;
    }
    bs.index.rev += 1;
    bs.save().unwrap_or_else(|e| die(format!("save: {e}")));
    println!("{{\"ok\":true,\"new_rev\":{}}}", bs.index.rev);
}

fn cmd_insert(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    let v = read_stdin_json();
    let expected_rev = v_u64(&v, "rev");
    ensure_rev(&bs, expected_rev);
    let after = v.get("after").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let code_str = v.get("code").and_then(|x| x.as_str()).unwrap_or_else(|| die("code missing")).to_string();
    let code = code_str.into_bytes();
    let tail = v_tail(&v);
    let (changed, unchanged) = v_purpose_decision(&v);
    if unchanged && bs.index.fileset.purpose.is_empty() { die("purpose_decision.unchanged=true 但 fileset.purpose 还未设置; 必须先 changed 或 meta 设一次"); }
    apply_purpose_decision(&mut bs, changed);

    let new_ulid_str = new_ulid();
    let insert_pos = if after == 0 { 0 } else {
        bs.index.blocks.iter().position(|b| b.seq == after).map(|p| p + 1).unwrap_or_else(|| die(format!("after seq {} not found", after)))
    };
    let mut datas: Vec<Vec<u8>> = Vec::with_capacity(bs.index.blocks.len() + 1);
    for b in &bs.index.blocks { datas.push(bs.block_bytes(b).to_vec()); }
    bs.index.blocks.insert(insert_pos, Block {
        ulid: new_ulid_str.clone(), seq: 0, byte_offset: 0, byte_length: 0,
        line_start: 0, line_end: 0, tail,
        symbols: split::extract_symbols(
            &code,
            split::SourceLanguage::detect(
                Path::new(&bs.index.fileset.path),
                Some(&bs.index.fileset.lang),
            )
            .unwrap_or_else(|error| die(error)),
        ),
    });
    datas.insert(insert_pos, code);
    bs.repack(datas);
    bs.index.rev += 1;
    bs.save().unwrap_or_else(|e| die(format!("save: {e}")));
    print_remap(&bs);
    if let Some((sim, warn)) = embed_check(&bs) { eprintln!("{}  (cos={:.3})", warn, sim); }
    let new_seq = insert_pos + 1;
    let (line_start, line_end, summary) = locked_block_span(&mut bs, new_seq);
    println!(
        "{{\"ok\":true,\"new_rev\":{},\"new_seq\":{},\"binding\":\"{}\",\"lines\":{{\"start\":{},\"end\":{}}},\"summary\":{}}}",
        bs.index.rev, new_seq, new_ulid_str, line_start, line_end, json_str(&summary)
    );
    eprintln!("locked lines {}-{}  (seq={} \"{}\")", line_start, line_end, new_seq, summary);
}

fn cmd_replace(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    let v = read_stdin_json();
    let expected_rev = v_u64(&v, "rev");
    ensure_rev(&bs, expected_rev);
    let seq = v_u64(&v, "seq") as usize;
    let code = v.get("code").and_then(|x| x.as_str()).unwrap_or_else(|| die("code missing")).to_string().into_bytes();
    let tail = v_tail(&v);
    let (changed, _) = v_purpose_decision(&v);
    apply_purpose_decision(&mut bs, changed);

    let pos = bs.index.blocks.iter().position(|b| b.seq == seq).unwrap_or_else(|| die(format!("seq {} not found", seq)));
    let ulid_kept = bs.index.blocks[pos].ulid.clone();
    let old_snap = deps::snapshot(&bs);
    bs.index.blocks[pos].tail = tail;
    let new_symbols = split::extract_symbols(
        &code,
        split::SourceLanguage::detect(
            Path::new(&bs.index.fileset.path),
            Some(&bs.index.fileset.lang),
        )
        .unwrap_or_else(|error| die(error)),
    );
    bs.index.blocks[pos].symbols = new_symbols;
    let mut datas: Vec<Vec<u8>> = bs.index.blocks.iter().map(|b| bs.block_bytes(b).to_vec()).collect();
    datas[pos] = code;
    bs.repack(datas);
    bs.index.rev += 1;
    // 跨块依赖告警(不阻断)
    let warns = deps::check_replace_impact(&old_snap, &bs, seq);
    if let Some(w) = deps::format_warns("replace", Some(seq), &warns) { eprintln!("{}", w); }
    bs.save().unwrap_or_else(|e| die(format!("save: {e}")));
    print_remap(&bs);
    if let Some((sim, warn)) = embed_check(&bs) { eprintln!("{}  (cos={:.3})", warn, sim); }
    // After repack, seq is renumbered to 1-based index; keep caller's seq if still present.
    let report_seq = bs.index.blocks.iter().find(|b| b.ulid == ulid_kept).map(|b| b.seq).unwrap_or(seq);
    let (line_start, line_end, summary) = locked_block_span(&mut bs, report_seq);
    println!(
        "{{\"ok\":true,\"new_rev\":{},\"seq\":{},\"binding\":\"{}\",\"lines\":{{\"start\":{},\"end\":{}}},\"summary\":{}}}",
        bs.index.rev, report_seq, ulid_kept, line_start, line_end, json_str(&summary)
    );
    eprintln!("locked lines {}-{}  (seq={} \"{}\")", line_start, line_end, report_seq, summary);
}

fn cmd_drop(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    let v = read_stdin_json();
    let expected_rev = v_u64(&v, "rev");
    ensure_rev(&bs, expected_rev);
    let seq = v_u64(&v, "seq") as usize;
    let (changed, _) = v_purpose_decision(&v);
    apply_purpose_decision(&mut bs, changed);

    let pos = bs.index.blocks.iter().position(|b| b.seq == seq).unwrap_or_else(|| die(format!("seq {} not found", seq)));
    let dropped_defines = bs.index.blocks[pos].symbols.defines.clone();
    let dropped = bs.index.blocks.remove(pos);
    let dropped_seq = dropped.seq;
    let dropped_ulid = dropped.ulid.clone();
    let dropped_summary = dropped.tail.summary.clone();
    bs.index.deleted.push(DeletedBlock {
        ulid: dropped.ulid.clone(), seq_was: dropped.seq,
        tail: dropped.tail.clone(), deleted_at_rev: bs.index.rev + 1,
        byte_length: dropped.byte_length,
    });
    let mut datas: Vec<Vec<u8>> = Vec::with_capacity(bs.index.blocks.len());
    for b in &bs.index.blocks { datas.push(bs.block_bytes(b).to_vec()); }
    bs.repack(datas);
    bs.index.rev += 1;
    // 跨块依赖告警: dropped 块的独占符号如果被别人引用 -> 提示
    let warns = deps::check_drop_impact(&dropped_defines, &bs);
    if let Some(w) = deps::format_warns("drop", None, &warns) { eprintln!("{}", w); }
    bs.save().unwrap_or_else(|e| die(format!("save: {e}")));
    print_remap(&bs);
    if let Some((sim, warn)) = embed_check(&bs) { eprintln!("{}  (cos={:.3})", warn, sim); }
    bs.recount_lines();
    println!(
        "{{\"ok\":true,\"new_rev\":{},\"dropped_seq_was\":{},\"binding\":\"{}\",\"lines\":{{\"start\":0,\"end\":0}},\"summary\":{}}}",
        bs.index.rev, dropped_seq, dropped_ulid, json_str(&dropped_summary)
    );
    eprintln!("dropped seq_was={} \"{}\" (block removed; remaining blocks remapped)", dropped_seq, dropped_summary);
}

/// Line span of the locked (edited) block after recount_lines.
fn locked_block_span(bs: &mut BlockSet, seq: usize) -> (usize, usize, String) {
    bs.recount_lines();
    if let Some(b) = bs.index.blocks.iter().find(|b| b.seq == seq) {
        (b.line_start, b.line_end, b.tail.summary.clone())
    } else {
        (0, 0, String::new())
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 在写命令回执中打印旧 seq -> 新 seq 的重编号映射 (drop/insert 后)
fn print_remap(bs: &BlockSet) {
    let blocks = &bs.index.blocks;
    if blocks.is_empty() { return; }
    // programmatic remap output (debug-friendly; AI 不必解析这个)
    eprintln!("remap: seqs now 1..{}; blocks ulid/summary:", blocks.len());
    for b in blocks { eprintln!("  seq={} ulid={} summary=\"{}\"", b.seq, b.ulid, b.tail.summary); }
}

fn cmd_assemble(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    bs.recount_lines();
    let output_arg = if args.len() >= 3 && args[1] == "-o" {
        args[2].as_str()
    } else {
        bs.index.fileset.path.as_str()
    };
    let out_path = workspace_path(output_arg).unwrap_or_else(|error| die(error));
    // Refuse wiping a real file with an empty blockset (calculator ui.json footgun).
    if bs.code.is_empty() && bs.index.blocks.is_empty() {
        let disk_len = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
        if disk_len > 0 {
            die(format!(
                "assemble refused: blockset for `{}` has 0 blocks/0 bytes but `{}` already has {disk_len} B on disk. \
                 Refusing to overwrite with empty projection. Delete this blockset or insert/split real content first.",
                bs.index.fileset.path,
                out_path.display()
            ));
        }
    }
    if let Err(e) = assemble::assemble_to(&mut bs, &out_path) { die(format!("assemble: {e}")); }
    bs.index.rev += 1;
    // 自增 rev 后, 写 line-map (它记录 assemble 后的 source 行 -> seq, 用当前 rev)
    let lm = LineMap::from_blockset(&bs);
    let lm_rev = lm.rev;
    if let Err(e) = lm.save(&bs.root, &bs.index.fileset.ulid) {
        eprintln!("WARN: write line-map failed: {e}");
    }
    bs.save().unwrap_or_else(|e| die(format!("save: {e}")));
    println!("assembled {} ({} blocks, {} B) -> {}  rev={}  linemap_rev={}",
        bs.index.fileset.path, bs.index.blocks.len(), bs.code.len(), out_path.display(), bs.index.rev, lm_rev);
    if let Some((sim, warn)) = embed_check(&bs) { eprintln!("{}  (cos={:.3})", warn, sim); }
}

fn cmd_verify(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let mut bs = bs_load(&args[0]);
    bs.recount_lines();
    // 不变式: blocks.vib.code == 各块 byte 范围拼接 (repacked 后恒为真). 直接 sha256 比对 source_sha256.
    let assembled = assemble::assemble_bytes(&bs);
    let cur_sha = sha256_hex(&assembled);
    if bs.index.fileset.source_sha256.is_empty() {
        eprintln!("(no source_sha256 on record; this blockset has never been assembled)");
    } else if cur_sha != bs.index.fileset.source_sha256 {
        die(format!(
            "INTERNAL FAIL: assembled sha {} != recorded {}",
            cur_sha, bs.index.fileset.source_sha256
        ));
    }
    // 与 assemble 输出文件 / 指定 original 比对
    let target = if args.len() >= 2 { PathBuf::from(&args[1]) } else { PathBuf::from(&bs.index.fileset.path) };
    if target.exists() {
        match fs::read(&target) {
            Ok(orig) => {
                if orig == assembled { println!("OK: byte-identical with {} ({} B)", target.display(), orig.len()); }
                else { eprintln!("MISMATCH vs {}: assembled {} B vs file {} B", target.display(), assembled.len(), orig.len()); exit(1); }
            }
            Err(e) => die(format!("read original: {e}")),
        }
    } else {
        println!("OK: invariant holds ({} blocks, {} B; no original file to compare)", bs.index.blocks.len(), assembled.len());
    }
}

/// vibe lookup <path> <line>  把源文件行号定位到区块 seq + 块内行号
/// 用途: LSP 诊断 / 运行时栈回溯 直接映射到区块, AI 不必读全文
fn cmd_lookup(args: &[String]) {
    if args.len() < 2 { usage(); exit(2); }
    let bs = bs_load(&args[0]);
    let line: usize = args[1].parse().unwrap_or_else(|e| die(format!("line: {e}")));
    let lm = LineMap::load(&bs.root, &bs.index.fileset.ulid)
        .unwrap_or_else(|e| die(format!("line-map missing; run assemble first: {e}")));
    let cur = bs.index.rev;
    if lm.rev != cur {
        eprintln!("WARN: line-map rev={} vs current rev={} (重新 assemble 同步)", lm.rev, cur);
    }
    match lm.lookup(line) {
        Some((seq, local)) => {
            let blk = bs.index.blocks.iter().find(|b| b.seq == seq);
            let summary = blk.map(|b| b.tail.summary.as_str()).unwrap_or("(block missing)");
            let purpose = blk.map(|b| b.tail.purpose.as_str()).unwrap_or("");
            println!("[line {}] -> seq={}  local_line={}", line, seq, local);
            println!("  summary: {}", summary);
            if !purpose.is_empty() {
                println!("  purpose: {}", purpose);
            }
            println!("  hint: vibe peek {} {}  | vibe read {} {}",
                bs.index.fileset.path, seq, bs.index.fileset.path, seq);
        }
        None => {
            eprintln!("line {} out of range (line_count={})", line, lm.line_count);
            exit(1);
        }
    }
}

/// vibe linemap <path>  dump line-map.json 内容, 调试用
fn cmd_linemap(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let bs = bs_load(&args[0]);
    let lm = LineMap::load(&bs.root, &bs.index.fileset.ulid)
        .unwrap_or_else(|e| die(format!("line-map missing; run assemble first: {e}")));
    println!("rev           : {}", lm.rev);
    println!("source_sha256 : {}", lm.source_sha256);
    println!("line_count    : {}", lm.line_count);
    println!("ranges ({}) :", lm.ranges.len());
    for r in &lm.ranges {
        println!("  seq={:>2}  lines {}-{}", r.seq, r.from, r.to);
    }
}

/// vibe deps <path>  dump 完整依赖图: 每个 seq -> defines, uses_in_fileset, depends_on(其它块的 defines)
fn cmd_deps(args: &[String]) {
    if args.is_empty() { usage(); exit(2); }
    let bs = bs_load(&args[0]);
    let fs_defines = deps::fileset_defines(&bs);
    println!("file: {}", bs.index.fileset.path);
    println!("purpose: {}", bs.index.fileset.purpose);
    println!("rev: {}", bs.index.rev);
    println!("analysis: heuristic lexical symbol graph (not scope-resolved)");
    println!("blocks: {}", bs.index.blocks.len());
    println!("fileset_defines: {}", fs_defines.iter().cloned().collect::<Vec<_>>().join(", "));
    println!();
    for b in &bs.index.blocks {
        println!("seq={}  {}", b.seq, b.tail.summary);
        if !b.symbols.defines.is_empty() {
            println!("  defines: {}", b.symbols.defines.join(", "));
        }
        let uses_in_file: Vec<String> = b.symbols.uses.iter()
            .filter(|u| fs_defines.contains(*u) && !b.symbols.defines.contains(u))
            .cloned().collect();
        if !uses_in_file.is_empty() {
            // 把 uses_in_file 对回 seq, 形成 depends_on 关系
            let mut dep_seqs: Vec<usize> = Vec::new();
            for sym in &uses_in_file {
                for ob in &bs.index.blocks {
                    if ob.seq == b.seq { continue; }
                    if ob.symbols.defines.contains(sym) && !dep_seqs.contains(&ob.seq) {
                        dep_seqs.push(ob.seq);
                    }
                }
            }
            println!("  uses_fileset: {}", uses_in_file.join(", "));
            println!("  depends_on  : seqs {:?}", dep_seqs);
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::workspace_path;
    use std::path::PathBuf;

    #[test]
    fn workspace_paths_are_lexically_confined() -> Result<(), String> {
        assert_eq!(
            workspace_path("./src/main.rs")?,
            PathBuf::from("src").join("main.rs")
        );
        assert!(workspace_path("../outside.rs").is_err());
        assert!(workspace_path("/tmp/outside.rs").is_err());
        #[cfg(windows)]
        assert!(workspace_path("C:\\outside.rs").is_err());
        Ok(())
    }
}