use crate::format::{sha256_hex, BlockSet};
use std::path::Path;

/// Marker line written into Python (hash-comment) projections for kids / LLM orientation.
/// Not stored inside blocks.vib.code — stripped on split/verify.
pub const VIBE_MARKER_PREFIX: &str = "# === vibe:seq=";

/// Pure block concatenation (source of truth).
pub fn assemble_bytes(bs: &BlockSet) -> Vec<u8> {
    bs.code.clone()
}

/// Whether this language gets `# === vibe:seq=... ===` markers in the disk projection.
pub fn uses_hash_line_markers(lang: &str) -> bool {
    matches!(
        lang.trim().to_ascii_lowercase().as_str(),
        "python" | "py" | "shell" | "bash" | "sh"
    )
}

pub fn ensure_trailing_newline(mut data: Vec<u8>) -> Vec<u8> {
    if !data.is_empty() && data.last() != Some(&b'\n') {
        data.push(b'\n');
    }
    data
}

pub fn is_vibe_marker_line(line: &[u8]) -> bool {
    let trimmed = trim_ascii_start(line);
    trimmed.starts_with(VIBE_MARKER_PREFIX.as_bytes())
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\r') {
        i += 1;
    }
    &bytes[i..]
}

/// Remove vibe marker lines so split/verify see pure code.
pub fn strip_vibe_markers(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len());
    let mut start = 0usize;
    while start <= src.len() {
        let end = src[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|p| start + p)
            .unwrap_or(src.len());
        let line = &src[start..end];
        let is_marker = is_vibe_marker_line(line);
        if !is_marker {
            out.extend_from_slice(line);
            if end < src.len() {
                out.push(b'\n');
            }
        } else if end < src.len() {
            // drop marker line including its newline
        }
        if end >= src.len() {
            break;
        }
        start = end + 1;
    }
    out
}

fn sanitize_purpose(purpose: &str) -> String {
    let flat: String = purpose
        .chars()
        .map(|c| if c.is_control() || c == '\n' || c == '\r' { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let flat = flat.replace("===", "-");
    if flat.chars().count() > 60 {
        flat.chars().take(60).collect::<String>() + "…"
    } else if flat.is_empty() {
        "(no purpose)".into()
    } else {
        flat
    }
}

pub fn marker_line(seq: usize, purpose: &str) -> Vec<u8> {
    format!(
        "{}{} purpose={} ===\n",
        VIBE_MARKER_PREFIX,
        seq,
        sanitize_purpose(purpose)
    )
    .into_bytes()
}

/// Disk projection: optional per-block marker lines + pure block bytes.
pub fn assemble_projection_bytes(bs: &BlockSet) -> Vec<u8> {
    if !uses_hash_line_markers(&bs.index.fileset.lang) {
        return assemble_bytes(bs);
    }
    let mut out = Vec::new();
    for blk in &bs.index.blocks {
        out.extend_from_slice(&marker_line(blk.seq, &blk.tail.purpose));
        out.extend_from_slice(bs.block_bytes(blk));
    }
    out
}

/// Write projection to disk; record source_sha256 of **pure** block concat (invariant).
pub fn assemble_to(bs: &mut BlockSet, out_path: &Path) -> std::io::Result<()> {
    let pure = assemble_bytes(bs);
    let projection = assemble_projection_bytes(bs);
    std::fs::write(out_path, &projection)?;
    bs.index.fileset.source_sha256 = sha256_hex(&pure);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_newline_and_strip_markers() {
        assert_eq!(ensure_trailing_newline(b"abc".to_vec()), b"abc\n");
        assert_eq!(ensure_trailing_newline(b"abc\n".to_vec()), b"abc\n");
        let src = b"# === vibe:seq=1 purpose=hi ===\ndef a():\n    return 1\n# === vibe:seq=2 purpose=x ===\ndef b():\n    return 2\n";
        let stripped = strip_vibe_markers(src);
        assert_eq!(stripped, b"def a():\n    return 1\ndef b():\n    return 2\n");
    }
}
