use crate::format::{BlockSet, sha256_hex};
use std::path::Path;

/// 拼接所有块(按 byte_offset 顺序)的字节. 不变式: blocks.vib.code 是块连续拼接.
pub fn assemble_bytes(bs: &BlockSet) -> Vec<u8> {
    bs.code.clone()
}

/// 写出源文件 + 同步 sha256 到 index. 不写 line-map (由 cmd_assemble 在 rev 自增后做). rev 由调用方管理.
pub fn assemble_to(bs: &mut BlockSet, out_path: &Path) -> std::io::Result<()> {
    let bytes = assemble_bytes(bs);
    std::fs::write(out_path, &bytes)?;
    bs.index.fileset.source_sha256 = sha256_hex(&bytes);
    Ok(())
}