//! Approximate token counts for context-window UI when the provider does not
//! return real `prompt_tokens`, or when it bills/reports by character (字).

/// True if `reported` looks like a character count rather than BPE/token count.
pub fn looks_like_char_count(reported: u64, char_count: u64) -> bool {
    if reported == 0 || char_count == 0 {
        return false;
    }
    // Within ~20% of raw character count → almost certainly counting 字/letters.
    let lo = char_count.saturating_mul(8) / 10;
    let hi = char_count.saturating_mul(12) / 10;
    reported >= lo && reported <= hi
}

fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified
        | '\u{3400}'..='\u{4DBF}' // CJK Ext A
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility
        | '\u{3000}'..='\u{303F}' // CJK punctuation
        | '\u{FF00}'..='\u{FFEF}' // fullwidth forms
    )
}

/// Estimate tokens for plain text: CJK ≈ 1 tok/char, Latin ≈ 4 chars/tok.
pub fn estimate_text_tokens(text: &str) -> u64 {
    let mut tokens = 0.0_f64;
    for ch in text.chars() {
        if ch.is_whitespace() {
            tokens += 0.15;
        } else if is_cjk(ch) {
            tokens += 1.0;
        } else if ch.is_ascii() {
            tokens += 0.25;
        } else {
            tokens += 0.5;
        }
    }
    tokens.ceil().max(0.0) as u64
}

pub fn count_chars(text: &str) -> u64 {
    text.chars().filter(|c| !c.is_whitespace()).count() as u64
}

/// Estimate tokens for a chat message list (content + tool call payloads).
pub fn estimate_messages_tokens(messages: &[crate::provider::Message]) -> u64 {
    let mut total = 0u64;
    for message in messages {
        // Per-message framing overhead (role / separators) — small constant.
        total = total.saturating_add(4);
        if let Some(content) = message.content.as_deref() {
            total = total.saturating_add(estimate_text_tokens(content));
        }
        if let Some(calls) = message.tool_calls.as_ref() {
            for call in calls {
                total = total.saturating_add(estimate_text_tokens(&call.function.name));
                total = total.saturating_add(estimate_text_tokens(&call.function.arguments));
                total = total.saturating_add(6);
            }
        }
        if let Some(id) = message.tool_call_id.as_deref() {
            total = total.saturating_add(estimate_text_tokens(id));
        }
    }
    // Tools schema is sent every turn; coarse allowance so the bar is not wildly low.
    total.saturating_add(256)
}

pub fn count_messages_chars(messages: &[crate::provider::Message]) -> u64 {
    let mut total = 0u64;
    for message in messages {
        if let Some(content) = message.content.as_deref() {
            total = total.saturating_add(count_chars(content));
        }
        if let Some(calls) = message.tool_calls.as_ref() {
            for call in calls {
                total = total.saturating_add(count_chars(&call.function.name));
                total = total.saturating_add(count_chars(&call.function.arguments));
            }
        }
    }
    total
}

/// Prefer provider `prompt_tokens` when it is a real token count; otherwise estimate.
pub fn resolve_prompt_tokens(reported: u64, messages: &[crate::provider::Message]) -> u64 {
    let estimated = estimate_messages_tokens(messages);
    let chars = count_messages_chars(messages);
    if reported == 0 {
        return estimated;
    }
    if looks_like_char_count(reported, chars) {
        return estimated;
    }
    reported
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Message;

    #[test]
    fn english_is_not_one_token_per_letter() {
        let text = "Hello world, this is an English sentence.";
        let chars = count_chars(text);
        let tokens = estimate_text_tokens(text);
        assert!(chars > 20);
        // Must be clearly fewer tokens than letters.
        assert!(tokens < chars / 2);
        assert!(tokens > 3);
    }

    #[test]
    fn cjk_roughly_one_token_per_char() {
        let text = "你好世界项目树节点";
        let tokens = estimate_text_tokens(text);
        let chars = count_chars(text);
        assert!(tokens >= chars.saturating_sub(2) && tokens <= chars + 2);
    }

    #[test]
    fn char_billing_is_detected_and_replaced() {
        let messages = vec![Message {
            role: "user".to_string(),
            content: Some("Hello world example text for token estimation".into()),
            tool_calls: None,
            tool_call_id: None,
        }];
        let chars = count_messages_chars(&messages);
        let fake_reported = chars; // provider counted letters
        assert!(looks_like_char_count(fake_reported, chars));
        let resolved = resolve_prompt_tokens(fake_reported, &messages);
        let content_tokens = estimate_text_tokens(messages[0].content.as_deref().unwrap_or(""));
        // Must not treat each Latin letter as one token.
        assert!(content_tokens < chars / 2);
        // Resolved window size should follow estimate, not raw letter count.
        assert_ne!(resolved, fake_reported);
        assert!(resolved >= content_tokens);
    }
}
