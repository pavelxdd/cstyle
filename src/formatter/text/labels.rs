//! Label prefixes in line text.

use crate::formatter::lexer::raw_strings;
use crate::formatter::text::line_scan::advance_quoted_literal;
use crate::formatter::text::line_scan::find_byte;
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{is_identifier_continue, is_identifier_start};

/// The colon ending a `case` or `default` label that starts `line`.
pub(crate) fn find_case_colon(line: &str) -> Option<usize> {
    let trimmed = line.trimmed_start();
    if !(is_case_label_start(trimmed) || is_default_label_start(trimmed)) {
        return None;
    }
    let start = line.len() - trimmed.len();
    find_case_colon_from(line, start)
}

pub(crate) fn is_case_label_start(line: &str) -> bool {
    line.strip_prefix("case")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|ch| !is_identifier_continue(ch))
}

pub(crate) fn is_default_label_start(line: &str) -> bool {
    line.strip_prefix("default").is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|ch| !is_identifier_continue(ch))
    })
}

/// The text after a user label at the start of `line`, or the trimmed line.
pub(crate) fn after_user_label(line: &str) -> &str {
    let trimmed = line.trimmed_start();
    let Some(colon) = find_byte(trimmed.as_bytes(), b':') else {
        return trimmed;
    };
    let (label, rest) = (&trimmed[..colon], &trimmed[colon + 1..]);
    if label.is_empty()
        || rest.starts_with(':')
        || matches!(
            label,
            "case" | "default" | "public" | "protected" | "private"
        )
        || label.contains(|ch: char| !is_identifier_continue(ch))
        || !label.starts_with(is_identifier_start)
    {
        return trimmed;
    }
    rest.trimmed_start()
}

pub(crate) fn find_case_colon_from(line: &str, start: usize) -> Option<usize> {
    // Every byte that matters is ASCII, and no byte of a wider character
    // equals one.
    let bytes = line.as_bytes();
    let mut index = start;
    while index < bytes.len() && !line.is_char_boundary(index) {
        index += 1;
    }
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut in_block_comment = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut ternary_contexts = Vec::new();

    while let Some(&byte) = bytes.get(index) {
        let next = bytes.get(index + 1).copied();

        if in_block_comment {
            if byte == b'*' && next == Some(b'/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if quote.is_some() {
            advance_quoted_literal(char::from(byte), &mut quote, &mut escaped);
            index += 1;
            continue;
        }

        if byte == b'/' && next == Some(b'/') {
            return None;
        }
        if byte == b'/' && next == Some(b'*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if matches!(byte, b'u' | b'L' | b'U' | b'R')
            && let Some(end) = raw_strings::end(line, index)
        {
            index = end;
            continue;
        }
        if byte == b'"' || (byte == b'\'' && !is_byte_digit_separator(bytes, index)) {
            quote = Some(char::from(byte));
            index += 1;
            continue;
        }
        match byte {
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = bracket_depth.saturating_sub(1),
            b'{' => brace_depth += 1,
            b'}' => brace_depth = brace_depth.saturating_sub(1),
            b'?' => ternary_contexts.push((paren_depth, bracket_depth, brace_depth)),
            b':' if next == Some(b':') => {
                index += 2;
                continue;
            }
            b':' if ternary_contexts.last().copied()
                == Some((paren_depth, bracket_depth, brace_depth)) =>
            {
                ternary_contexts.pop();
            }
            b':' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                return Some(index);
            }
            _ => {}
        }
        index += 1;
    }

    None
}

pub(crate) fn is_byte_digit_separator(bytes: &[u8], index: usize) -> bool {
    index > 0
        && bytes[index - 1].is_ascii_hexdigit()
        && bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit)
}
