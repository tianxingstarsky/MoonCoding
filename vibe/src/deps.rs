use crate::format::BlockSet;
use std::collections::BTreeSet;

/// 把 BlockSet 的 defines/uses 转成 (seq -> (Vec defining_names, Vec using_names)) 摘要
pub struct BlockSym {
    pub seq: usize,
    pub summary: String,
    pub defines: Vec<String>,
    pub uses: Vec<String>,
}

pub fn snapshot(bs: &BlockSet) -> Vec<BlockSym> {
    bs.index.blocks.iter().map(|b| BlockSym {
        seq: b.seq,
        summary: b.tail.summary.clone(),
        defines: b.symbols.defines.clone(),
        uses: b.symbols.uses.clone(),
    }).collect()
}

/// 文件级 defines 集合
pub fn fileset_defines(bs: &BlockSet) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for b in &bs.index.blocks {
        for d in &b.symbols.defines { s.insert(d.clone()); }
    }
    s
}

/// replace 后告警: 计算被移除的符号(旧文件级 defines - 新文件级 defines),
/// 扫所有 *其它* 块的 uses, 凡引用被移除符号的块 -> WARN.
/// 返回 Vec<(seq, summary, removed_symbols_used)>
pub fn check_replace_impact(
    old_snapshot: &[BlockSym],
    new_bs: &BlockSet,
    replaced_seq: usize,
) -> Vec<(usize, String, Vec<String>)> {
    let old_fileset: BTreeSet<String> = old_snapshot.iter()
        .flat_map(|b| b.defines.iter().cloned()).collect();
    let new_fileset: BTreeSet<String> = fileset_defines(new_bs);
    let removed: BTreeSet<String> = old_fileset.difference(&new_fileset).cloned().collect();
    if removed.is_empty() { return Vec::new(); }

    let mut warns: Vec<(usize, String, Vec<String>)> = Vec::new();
    for b in &new_bs.index.blocks {
        if b.seq == replaced_seq { continue; }
        let used_here: Vec<String> = b.symbols.uses.iter()
            .filter(|u| removed.contains(*u))
            .cloned()
            .collect();
        if !used_here.is_empty() {
            warns.push((b.seq, b.tail.summary.clone(), used_here));
        }
    }
    warns
}

/// drop 后告警: 被删块 defines 中那些在新文件级 defines 里也已不存在的 (即独占符号),
/// 扫剩余块的 uses
pub fn check_drop_impact(
    dropped_defines: &[String],
    new_bs: &BlockSet,
) -> Vec<(usize, String, Vec<String>)> {
    let new_fileset = fileset_defines(new_bs);
    let removed: Vec<String> = dropped_defines.iter()
        .filter(|d| !new_fileset.contains(*d))
        .cloned()
        .collect();
    if removed.is_empty() { return Vec::new(); }
    let removed_set: BTreeSet<String> = removed.into_iter().collect();

    let mut warns: Vec<(usize, String, Vec<String>)> = Vec::new();
    for b in &new_bs.index.blocks {
        let used_here: Vec<String> = b.symbols.uses.iter()
            .filter(|u| removed_set.contains(*u))
            .cloned()
            .collect();
        if !used_here.is_empty() {
            warns.push((b.seq, b.tail.summary.clone(), used_here));
        }
    }
    warns
}

/// 把告警列表打印成可读 WARN 字符串, 给 AI 一眼定位
pub fn format_warns(operation: &str, target_seq: Option<usize>, warns: &[(usize, String, Vec<String>)]) -> Option<String> {
    if warns.is_empty() { return None; }
    let mut out = String::new();
    out.push_str(&format!("WARN: cross-block dep impact ({}):", operation));
    if let Some(s) = target_seq {
        out.push_str(&format!(" block seq={} changed ->", s));
    }
    out.push('\n');
    for (seq, summary, syms) in warns {
        out.push_str(&format!("  seq={} \"{}\" uses now-removed symbol(s): {}\n",
            seq, summary, syms.join(", ")));
    }
    out.push_str("  -> check those blocks; use `vibe peek <path> <seq>` to review.");
    Some(out)
}