use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ulid::Ulid;

/// 程序内部生成新 ULID(基于系统时间+随机,跨文件唯一)
pub fn new_ulid() -> String {
    Ulid::new().to_string()
}

/// ======== 存储层 (AI 永不见, 由 #CX 管理) ========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tail {
    /// 代码功能: 口语化叙事 (函数名+传参+用法+实现)  -- #AI
    pub purpose: String,
    /// 功能简介: 带函数名和传参签名行  -- #AI
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// 绑定ID: ULID, 永不变  -- #CX
    pub ulid: String,
    /// 顺序编号: 物理顺序, 写出/插入/drop 时自动重编号  -- #CX
    pub seq: usize,
    /// 在 blocks.vib.code 中的字节偏移  -- #CX
    pub byte_offset: u64,
    /// 在 blocks.vib.code 中的字节长度  -- #CX
    pub byte_length: u64,
    /// 头索引的"关联行数"不持久化: 每次读取时程序实时从代码主体累加填充  -- #CX runtime
    #[serde(skip)]
    pub line_start: usize,
    #[serde(skip)]
    pub line_end: usize,
    /// 尾索引: 全 #AI
    pub tail: Tail,
    /// 符号表 #CX derived (split / replace / insert 时刷新)
    /// defines: 该块定义的函数/类名
    /// uses: 该块字节内出现的 identifier(已去重, 不含语言关键字)
    #[serde(default)]
    pub symbols: Symbols,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Symbols {
    pub defines: Vec<String>,
    pub uses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fileset {
    pub ulid: String,                 // #CX
    pub name: String,                 // #AI
    pub path: String,                 // #AI  POSIX 相对路径, 内部统一
    pub lang: String,                 // #AI
    pub purpose: String,              // #AI
    /// 功能细分: 汇总 blocks[*].tail.summary, 写时刷新  -- #CX derived
    pub breakdown: Vec<String>,
    /// 最近一次 assemble 写出的源文件 sha256  -- #CX
    pub source_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedBlock {
    pub ulid: String,
    pub seq_was: usize,
    pub tail: Tail,
    pub deleted_at_rev: u64,
    pub byte_length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    /// 单调递增版本号, AI 用它对账  -- #CX
    pub rev: u64,
    pub fileset: Fileset,
    pub blocks: Vec<Block>,
    pub deleted: Vec<DeletedBlock>,
}

/// 一个区块集在磁盘上的目录布局:
///   <root>/.vibe/<fileset_ulid>.vibe/
///       index.json
///       blocks.vib.code
pub struct BlockSet {
    pub root: PathBuf,
    pub index: Index,
    pub code: Vec<u8>,
}

impl BlockSet {
    pub fn dir_of(root: &Path, ulid: &str) -> PathBuf {
        root.join(".vibe").join(format!("{}.vibe", ulid))
    }

    pub fn index_path_of(root: &Path, ulid: &str) -> PathBuf {
        Self::dir_of(root, ulid).join("index.json")
    }

    pub fn code_path_of(root: &Path, ulid: &str) -> PathBuf {
        Self::dir_of(root, ulid).join("blocks.vib.code")
    }

    pub fn linemap_path_of(root: &Path, ulid: &str) -> PathBuf {
        Self::dir_of(root, ulid).join("line-map.json")
    }

    /// 在 root/.vibe 下扫描所有区块集, 按 path 字段建立索引, 返回 (path -> ulid) 表
    pub fn scan_index(root: &Path) -> io::Result<Vec<(String, String)>> {
        let vibe_root = root.join(".vibe");
        if !vibe_root.is_dir() { return Ok(Vec::new()); }
        let mut out: Vec<(String, String)> = Vec::new();
        for e in fs::read_dir(&vibe_root)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".vibe") { continue; }
            let ulid = name.trim_end_matches(".vibe").to_string();
            let idx_path = e.path().join("index.json");
            if !idx_path.is_file() { continue; }
            if let Ok(json) = fs::read_to_string(&idx_path) {
                if let Ok(idx) = serde_json::from_str::<Index>(&json) {
                    out.push((idx.fileset.path.clone(), ulid));
                }
            }
        }
        Ok(out)
    }

    /// 根据 path 锁定区块集并加载. 找不到返回 None.
    pub fn load_by_path(root: &Path, path: &str) -> io::Result<Option<Self>> {
        let table = Self::scan_index(root)?;
        for (p, ulid) in table {
            if p == path {
                return Self::load(root, &ulid).map(Some);
            }
        }
        Ok(None)
    }

    pub fn load(root: &Path, ulid: &str) -> io::Result<Self> {
        let _dir = Self::dir_of(root, ulid);
        let idx_text = fs::read_to_string(Self::index_path_of(root, ulid))?;
        let mut index: Index = serde_json::from_str(&idx_text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("index.json decode: {e}")))?;
        let code = fs::read(Self::code_path_of(root, ulid))?;
        // 持久化字段加载完成; 运行时字段(line_start/line_end)稍后由 recount_lines 填充
        for b in &mut index.blocks { b.line_start = 0; b.line_end = 0; }
        Ok(Self { root: root.to_path_buf(), index, code })
    }

    pub fn save(&self) -> io::Result<()> {
        let dir = Self::dir_of(&self.root, &self.index.fileset.ulid);
        fs::create_dir_all(&dir)?;
        let txt = serde_json::to_string_pretty(&self.index)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(Self::index_path_of(&self.root, &self.index.fileset.ulid), txt)?;
        fs::write(Self::code_path_of(&self.root, &self.index.fileset.ulid), &self.code)?;
        Ok(())
    }

    /// 取第 seq 块的字节切片(从 code 里按 byte_offset/byte_length 切)
    pub fn block_bytes(&self, blk: &Block) -> &[u8] {
        let s = blk.byte_offset as usize;
        let e = s + blk.byte_length as usize;
        &self.code[s..e]
    }

    /// 重新计算每块的 line_start / line_end (基于 byte 累加 + 当前状态)
    pub fn recount_lines(&mut self) {
        let mut line_no = 1usize;
        for blk in &mut self.index.blocks {
            let bytes = {
                let s = blk.byte_offset as usize;
                let e = s + blk.byte_length as usize;
                &self.code[s..e]
            };
            let nls = bytes.iter().filter(|&&b| b == b'\n').count();
            blk.line_start = line_no;
            // 末尾无换行也算一行. total = 该块覆盖的行数(1-based).
            let total = nls + if !bytes.is_empty() && *bytes.last().unwrap() != b'\n' { 1 } else { 0 };
            blk.line_end = line_no + total.saturating_sub(1);
            line_no += total;
        }
    }

    /// 重新打包 code 字节区: 把 blocks[*].data 顺序拼接, 更新 byte_offset/byte_length/seq
    pub fn repack(&mut self, block_datas: Vec<Vec<u8>>) {
        self.code = Vec::new();
        let mut off = 0u64;
        for (i, data) in block_datas.into_iter().enumerate() {
            let len = data.len() as u64;
            if i < self.index.blocks.len() {
                let blk = &mut self.index.blocks[i];
                blk.seq = i + 1;
                blk.byte_offset = off;
                blk.byte_length = len;
            }
            self.code.extend_from_slice(&data);
            off += len;
        }
        self.refresh_breakdown();
    }

    /// 刷新 fileset.breakdown: 汇总所有未删块 tail.summary  -- #CX derived (你的拍板 B)
    pub fn refresh_breakdown(&mut self) {
        self.index.fileset.breakdown = self.index.blocks.iter()
            .map(|b| b.tail.summary.clone())
            .collect();
    }
}

/// sha256 hex
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out.iter() {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

/// ======== line-map.json (assemble 后生成的源文件行号 -> 区块 seq 的二分查找表) ========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRange {
    pub seq: usize,
    pub from: usize,   // 1-based 起始行 (闭)
    pub to: usize,     // 1-based 结束行 (闭)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineMap {
    pub rev: u64,
    pub source_sha256: String,
    pub line_count: usize,
    pub ranges: Vec<LineRange>,
}

impl LineMap {
    pub fn from_blockset(bs: &BlockSet) -> Self {
        let mut ranges = Vec::with_capacity(bs.index.blocks.len());
        let mut line_count = 0usize;
        for b in &bs.index.blocks {
            let from = b.line_start.max(1);
            let to = b.line_end.max(from);
            ranges.push(LineRange { seq: b.seq, from, to });
            if to > line_count { line_count = to; }
        }
        LineMap {
            rev: bs.index.rev, // assemble 后会自增 rev 再写 index.json, 顺序见 cmd_assemble
            source_sha256: bs.index.fileset.source_sha256.clone(),
            line_count,
            ranges,
        }
    }

    pub fn save(&self, root: &Path, ulid: &str) -> io::Result<()> {
        let p = BlockSet::linemap_path_of(root, ulid);
        let txt = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(p, txt)
    }

    pub fn load(root: &Path, ulid: &str) -> io::Result<Self> {
        let p = BlockSet::linemap_path_of(root, ulid);
        let txt = fs::read_to_string(&p)?;
        serde_json::from_str(&txt).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("line-map decode: {e}")))
    }

    /// 二分查找 line 落在哪个 range. 返回 (seq, local_line).
    pub fn lookup(&self, line: usize) -> Option<(usize, usize)> {
        if line == 0 || line > self.line_count { return None; }
        let mut lo = 0isize;
        let mut hi = self.ranges.len() as isize - 1;
        while lo <= hi {
            let mid = ((lo + hi) / 2) as usize;
            let r = &self.ranges[mid];
            if line < r.from { hi = mid as isize - 1; }
            else if line > r.to { lo = mid as isize + 1; }
            else { return Some((r.seq, line - r.from + 1)); }
        }
        None
    }
}