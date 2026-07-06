use crate::format::{Block, BlockSet, Fileset, Index, Symbols, Tail, new_ulid};
use tree_sitter::{Language, Node, Parser};

fn lang_python() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

/// 从 Python 源字节抽取 (defines, uses):
///   defines: tree-sitter AST 里所有 function_definition / class_definition 的 name 字段
///   uses:    源里所有 identifier (regex), 去重, 排除语言关键字
pub fn extract_symbols(src: &[u8]) -> Symbols {
    let mut defines: Vec<String> = Vec::new();
    let mut parser = Parser::new();
    if parser.set_language(&lang_python()).is_ok() {
        if let Some(tree) = parser.parse(src, None) {
            collect_defines(tree.root_node(), src, &mut defines);
        }
    }
    // uses: 抽所有 [A-Za-z_][A-Za-z0-9_]* 去重 (排除关键字)
    let uses = extract_identifiers(src);
    Symbols { defines, uses }
}

fn collect_defines(node: Node, src: &[u8], out: &mut Vec<String>) {
    let kind = node.kind();
    if kind == "function_definition" || kind == "class_definition" {
        if let Some(n) = node.child_by_field_name("name") {
            if let Ok(t) = n.utf8_text(src) {
                let t = t.to_string();
                if !t.is_empty() && !out.contains(&t) { out.push(t); }
            }
        }
    }
    let mut i: u32 = 0;
    while let Some(c) = node.child(i) {
        collect_defines(c, src, out);
        i += 1;
    }
}

const PY_KEYWORDS: &[&str] = &[
    "False","None","True","and","as","assert","async","await","break","class","continue",
    "def","del","elif","else","except","finally","for","from","global","if","import","in",
    "is","lambda","nonlocal","not","or","pass","raise","return","try","while","with","yield",
    "self","cls",
];

fn extract_identifiers(src: &[u8]) -> Vec<String> {
    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let text = String::from_utf8_lossy(src);
    let mut cur = String::new();
    let mut in_str: Option<u8> = None; // ' or " tracking
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        // 字符串跨行不处理(简化: 遇到 ' 或 " 跳到匹配, 不抽其中标识符)
        if let Some(q) = in_str {
            if b == q { in_str = None; }
            // 处理 \" 转义
            if b == b'\\' && i + 1 < bytes.len() { i += 2; continue; }
            i += 1;
            continue;
        }
        if b == b'#' {
            // 注释: 跳到行尾
            while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
            continue;
        }
        if b == b'\'' || b == b'"' {
            in_str = Some(b);
            i += 1;
            continue;
        }
        if b.is_ascii_alphabetic() || b == b'_' {
            cur.clear();
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                cur.push(bytes[i] as char);
                i += 1;
            }
            if !PY_KEYWORDS.contains(&cur.as_str()) {
                set.insert(cur.clone());
            }
            continue;
        }
        i += 1;
    }
    set.into_iter().collect()
}

/// 找 src 中第一非空白字节的行首位置(用于"该块字节起点的相对偏移")
fn first_nonblank_line_bytes(data: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut i = 0usize;
    while i < data.len() {
        if data[i] == b'\n' {
            let mut e = i;
            while e > start && (data[e - 1] == b'\n' || data[e - 1] == b'\r') { e -= 1; }
            if e > start { return &data[start..e]; }
            start = i + 1;
        }
        i += 1;
    }
    let mut e = data.len();
    while e > start && (data[e - 1] == b'\n' || data[e - 1] == b'\r') { e -= 1; }
    &data[start..e]
}

/// 把 src 拆为函数级块. 不变式: 全块按序拼接 == 原文件.
///
/// 拆分策略(与行级实现保持行为兼容, AST 后更精确):
///   - root 的 named_children 顺序遍历
///   - 遇见 decorated_definition / function_definition / class_definition 时, 把它起始字节位置标记为"新块边界"
///   - 之后所有非 def 语句(紧跟在此 def 之后的 imports / 赋值 / if __name__ / 等)继续走当前段, 归到上一个 def 块
///   - 文件开头到第一个 def 之前 = module_header 块
///   - 没有 def 时 = 只有一个 module_header 块(整个文件)
pub fn split_python(src: &[u8], root: &std::path::Path, posix_path: &str, name: &str, purpose: &str) -> BlockSet {
    let mut parser = Parser::new();
    parser.set_language(&lang_python())
        .expect("tree-sitter python language load failed");

    let tree = parser.parse(src, None)
        .expect("tree-sitter parse failed");
    let mut boundaries: Vec<usize> = vec![0]; // byte offsets

    // tree-sitter 节点 kind 字符串(参见 tree-sitter-python grammar)
    let root_node = tree.root_node();

    // 遍历 root 的直接子节点(named_children). 跳过 comment/extra节点的 named_child(注释是 anonymous, 不计入 named_children).
    let mut cursor = root_node.walk();
    for child in root_node.named_children(&mut cursor) {
        let kind = child.kind();
        // decorated_definition 包含 @deco + 紧跟 def/class
        if kind == "decorated_definition"
            || kind == "function_definition"
            || kind == "class_definition"
        {
            boundaries.push(child.start_byte());
        }
    }

    boundaries.push(src.len());

    let fs_ulid = new_ulid();
    let mut blocks: Vec<Block> = Vec::new();
    let mut code = Vec::new();
    let mut off = 0u64;
    let mut seq = 1usize;

    for i in 0..boundaries.len() - 1 {
        let start = boundaries[i];
        let end = boundaries[i + 1];
        if end <= start { continue; }
        let data = &src[start..end];
        if data.is_empty() { continue; }

        // tail.summary: 块首非空行 作为预填(#AI 可后续 review 改)
        let sig = first_nonblank_line_bytes(data);
        let summary = if sig.is_empty() {
            format!("(seq {})", seq)
        } else {
            String::from_utf8_lossy(sig).into_owned()
        };

        let len = data.len() as u64;
        let symbols = extract_symbols(data);
        blocks.push(Block {
            ulid: new_ulid(),
            seq,
            byte_offset: off,
            byte_length: len,
            line_start: 0, line_end: 0,
            tail: Tail { purpose: String::new(), summary },
            symbols,
        });
        code.extend_from_slice(data);
        off += len;
        seq += 1;
    }

    let fileset = Fileset {
        ulid: fs_ulid.clone(),
        name: name.to_string(),
        path: posix_path.to_string(),
        lang: "python".to_string(),
        purpose: purpose.to_string(),
        breakdown: blocks.iter().map(|b| b.tail.summary.clone()).collect(),
        source_sha256: String::new(),
    };
    let index = Index { rev: 1, fileset, blocks, deleted: Vec::new() };
    let mut bs = BlockSet { root: root.to_path_buf(), index, code };
    bs.recount_lines();
    bs
}