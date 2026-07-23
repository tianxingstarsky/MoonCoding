//! Decode subprocess / console bytes. On Chinese Windows, tools often emit GBK.
//! Also helpers for streaming UTF-8 that must not split multi-byte characters.
use std::borrow::Cow;

/// Decode a streaming UTF-8 byte buffer without inventing U+FFFD mid-character.
///
/// Returns `(decoded_text, bytes_consumed)`. Trailing incomplete UTF-8 sequences
/// are left unconsumed (`error_len() == None`) so the caller can wait for more
/// network bytes. Hard-invalid bytes are skipped one at a time so the stream
/// can resynchronize (should be rare on HTTPS SSE).
pub fn take_valid_utf8_prefix(bytes: &[u8]) -> (String, usize) {
    if bytes.is_empty() {
        return (String::new(), 0);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_string(), bytes.len()),
        Err(err) => {
            let valid = err.valid_up_to();
            if valid > 0 {
                // SAFETY: valid_up_to guarantees a valid UTF-8 prefix.
                let text = std::str::from_utf8(&bytes[..valid])
                    .expect("valid_up_to prefix must be UTF-8")
                    .to_string();
                return (text, valid);
            }
            // valid == 0: either incomplete leading sequence, or invalid lead byte.
            if err.error_len().is_none() {
                // Need more bytes — do not consume.
                return (String::new(), 0);
            }
            // Skip one bad byte and let the caller try again with the rest.
            (String::new(), 1)
        }
    }
}

/// Decode command or console output into a displayable Unicode string.
pub fn decode_console_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    // Strict UTF-8 wins always — never reinterpret valid UTF-8 as GBK.
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }

    let utf8_lossy = String::from_utf8_lossy(bytes);
    let utf8_bad = count_replacements(&utf8_lossy);

    #[cfg(windows)]
    if let Some(gbk) = try_decode_gbk(bytes) {
        let gbk_bad = count_replacements(&gbk);
        let gbk_cjk = count_cjk(&gbk);
        let lossy_cjk = count_cjk(&utf8_lossy);
        // Only take GBK when it is clearly healthier AND yields more CJK than lossy UTF-8.
        if gbk_bad < utf8_bad && gbk_cjk > lossy_cjk {
            return gbk;
        }
    }

    utf8_lossy.into_owned()
}

fn count_replacements(text: &str) -> usize {
    text.chars().filter(|ch| *ch == '\u{FFFD}').count()
}

fn count_cjk(text: &str) -> usize {
    text.chars()
        .filter(|ch| {
            let c = *ch as u32;
            (0x4E00..=0x9FFF).contains(&c)
                || (0x3400..=0x4DBF).contains(&c)
                || (0xF900..=0xFAFF).contains(&c)
        })
        .count()
}

fn try_decode_gbk(bytes: &[u8]) -> Option<String> {
    decode_with_codepage(bytes, 936).or_else(|| decode_with_codepage(bytes, 54936))
}

#[cfg(windows)]
fn decode_with_codepage(bytes: &[u8], codepage: u32) -> Option<String> {
    use std::ptr;

    if bytes.is_empty() {
        return Some(String::new());
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn MultiByteToWideChar(
            code_page: u32,
            flags: u32,
            multi_byte_str: *const u8,
            cb_multi_byte: i32,
            wide_char_str: *mut u16,
            cch_wide_char: i32,
        ) -> i32;
    }

    const MB_ERR_INVALID_CHARS: u32 = 0x0000_0008;
    // SAFETY: Win32 MultiByteToWideChar with caller-owned buffers.
    unsafe {
        let needed = MultiByteToWideChar(
            codepage,
            MB_ERR_INVALID_CHARS,
            bytes.as_ptr(),
            bytes.len() as i32,
            ptr::null_mut(),
            0,
        );
        if needed <= 0 {
            return None;
        }
        let mut wide = vec![0u16; needed as usize];
        let written = MultiByteToWideChar(
            codepage,
            MB_ERR_INVALID_CHARS,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            needed,
        );
        if written <= 0 {
            return None;
        }
        wide.truncate(written as usize);
        // Strict UTF-16 — do not inject U+FFFD via lossy conversion.
        String::from_utf16(&wide).ok()
    }
}

#[cfg(not(windows))]
fn decode_with_codepage(_bytes: &[u8], _codepage: u32) -> Option<String> {
    None
}

/// Helper for callers that already hold a Cow from lossy UTF-8.
#[allow(dead_code)]
pub fn prefer_console_decode(bytes: &[u8], utf8_lossy: Cow<'_, str>) -> String {
    let replacements = count_replacements(&utf8_lossy);
    if replacements == 0 {
        return utf8_lossy.into_owned();
    }
    decode_console_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_valid_utf8() {
        assert_eq!(decode_console_bytes("中文 ok".as_bytes()), "中文 ok");
    }

    #[test]
    fn stream_prefix_holds_incomplete_multibyte() {
        let full = "你好世界".as_bytes();
        // Split inside the first CJK character (3-byte UTF-8).
        let (first, n) = take_valid_utf8_prefix(&full[..1]);
        assert_eq!(first, "");
        assert_eq!(n, 0);

        let mut buf = full[..1].to_vec();
        buf.extend_from_slice(&full[1..]);
        let (text, consumed) = take_valid_utf8_prefix(&buf);
        assert_eq!(text, "你好世界");
        assert_eq!(consumed, buf.len());
        assert!(!text.contains('\u{FFFD}'));
    }

    #[test]
    fn stream_prefix_across_chunk_boundary() {
        let full = "保存成功".as_bytes();
        let mut pending = Vec::new();
        let mut out = String::new();
        // Feed one byte at a time — the old from_utf8_lossy path would inject U+FFFD.
        for b in full {
            pending.push(*b);
            loop {
                let (piece, n) = take_valid_utf8_prefix(&pending);
                if n == 0 {
                    break;
                }
                out.push_str(&piece);
                pending.drain(..n);
                if piece.is_empty() && n == 1 {
                    // skipped invalid — continue
                    continue;
                }
            }
        }
        assert_eq!(out, "保存成功");
        assert!(pending.is_empty());
    }

    #[test]
    fn does_not_reinterpret_valid_utf8_as_gbk() {
        // Valid UTF-8 for "系统找不到指定的路径"
        let utf8 = "error: 系统找不到指定的路径。 (os error 3)".as_bytes();
        let text = decode_console_bytes(utf8);
        assert!(text.contains("系统"));
        assert!(!text.contains('\u{FFFD}'));
    }

    #[test]
    #[cfg(windows)]
    fn decodes_gbk_when_not_utf8() {
        // "错误" in GBK
        let gbk = [0xB4u8, 0xED, 0xCE, 0xF3];
        let text = decode_console_bytes(&gbk);
        assert_eq!(text, "错误");
        assert!(!text.contains('\u{FFFD}'));
    }
}
