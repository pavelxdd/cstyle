use super::token::{CommentKind, Token, token_text, tokenize};
use crate::source::lex::is_digit_separator;

pub(super) fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("*/")
}

pub(super) fn is_comment_only_line(line: &str) -> bool {
    line.starts_with("//")
        || line.starts_with("/*")
        || line.starts_with("*/")
        || line == "*"
        || line.starts_with("* ")
        || line.starts_with("*\t")
}

pub(super) fn has_unclosed_delimiter_after(text: &str, open: &str, close: &str) -> bool {
    text.rfind(open).is_some_and(|open_index| {
        text.rfind(close)
            .is_none_or(|close_index| close_index < open_index)
    })
}

pub(super) fn trailing_matching_parens(line: &str) -> Option<(usize, usize)> {
    let close_pos = line.char_indices().next_back()?.0;
    let mut open_stack: Vec<usize> = Vec::new();
    let mut in_quote = false;
    let mut quote = '\0';
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_quote = false;
            }
            continue;
        }
        if matches!(ch, '"' | '\'') {
            in_quote = true;
            quote = ch;
            escaped = false;
            continue;
        }
        match ch {
            '(' => open_stack.push(index),
            ')' => {
                let open_pos = open_stack.pop()?;
                if index == close_pos {
                    return Some((open_pos, close_pos));
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn has_top_level_comma_in_text(text: &str) -> bool {
    let mut depth = 0usize;
    let mut in_quote = false;
    let mut quote = '\0';
    let mut escaped = false;
    for ch in text.chars() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_quote = false;
            }
            continue;
        }
        if matches!(ch, '"' | '\'') {
            in_quote = true;
            quote = ch;
            escaped = false;
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

pub(super) fn line_ends_with_comment(line: &str) -> bool {
    let trimmed = line.trim_end();
    if trimmed.ends_with("*/") {
        return true;
    }
    let bytes = trimmed.as_bytes();
    let mut index = 0;
    let mut in_string = false;
    let mut in_char = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if in_string || in_char => {
                index += 2;
                continue;
            }
            b'"' if !in_char => in_string = !in_string,
            b'\'' if !in_string => in_char = !in_char,
            b'/' if !in_string
                && !in_char
                && index + 1 < bytes.len()
                && bytes[index + 1] == b'/' =>
            {
                return true;
            }
            _ => {}
        }
        index += 1;
    }
    false
}

pub(super) fn find_outside_quotes(line: &str, needle: &str) -> Option<usize> {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if !in_string && !in_char && line[index..].starts_with(needle) {
            return Some(index);
        }
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && (in_string || in_char) {
            escaped = true;
            continue;
        }
        if ch == '"' && !in_char {
            in_string = !in_string;
        } else if ch == '\'' && !in_string {
            in_char = !in_char;
        }
    }
    None
}

pub(super) fn unmatched_open_paren_column(line: &str) -> Option<usize> {
    unmatched_open_paren_columns(line)
        .into_iter()
        .rev()
        .find(|&column| line[column + 1..].chars().any(|ch| !ch.is_whitespace()))
}

pub(super) fn unmatched_open_bracket_column(line: &str) -> Option<usize> {
    unmatched_open_paren_columns(line)
        .into_iter()
        .rev()
        .find(|&column| line[column..].starts_with('['))
}

pub(super) fn line_paren_imbalance(line: &str) -> (usize, Vec<usize>) {
    let chars = line.chars().collect::<Vec<_>>();
    let mut stack: Vec<usize> = Vec::new();
    let mut unmatched_closes = 0usize;
    let mut index = 0;
    let mut column = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
                column += 2;
            } else {
                index += 1;
                column += ch.len_utf8();
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            column += ch.len_utf8();
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            column += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            column += ch.len_utf8();
            continue;
        }
        match ch {
            '(' | '[' => stack.push(column),
            ')' | ']' if stack.pop().is_none() => unmatched_closes += 1,
            ')' | ']' => {}
            _ => {}
        }
        index += 1;
        column += ch.len_utf8();
    }
    (unmatched_closes, stack)
}

/// Returns the brace imbalance of a single line as `(unmatched_closes, unmatched_opens)`,
/// ignoring braces inside strings and comments. A `}` without a matching `{` earlier on the
/// same line counts as an unmatched close; a `{` left open at the end counts as an open.
pub(super) fn line_brace_imbalance(line: &str) -> (usize, usize) {
    let chars = line.chars().collect::<Vec<_>>();
    let mut open_depth = 0usize;
    let mut unmatched_closes = 0usize;
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        match ch {
            '{' => open_depth += 1,
            '}' if open_depth > 0 => open_depth -= 1,
            '}' => unmatched_closes += 1,
            _ => {}
        }
        index += 1;
    }
    (unmatched_closes, open_depth)
}

/// True when the line has a `{` or `}` outside strings and comments.
pub(super) fn line_has_brace(line: &str) -> bool {
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if ch == '{' || ch == '}' {
            return true;
        }
        index += 1;
    }
    false
}

pub(super) fn unmatched_open_paren_columns(line: &str) -> Vec<usize> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut stack = Vec::new();
    let mut index = 0;
    let mut column = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();

        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
                column += 2;
            } else {
                index += 1;
                column += ch.len_utf8();
            }
            continue;
        }

        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            column += ch.len_utf8();
            continue;
        }

        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            column += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            column += ch.len_utf8();
            continue;
        }

        match ch {
            '(' | '[' => stack.push(column),
            ')' | ']' => {
                stack.pop();
            }
            _ => {}
        }
        index += 1;
        column += ch.len_utf8();
    }

    stack
}

pub(super) fn last_unmatched_open_delimiter(line: &str) -> Option<(char, usize)> {
    let indexed = line.char_indices().collect::<Vec<_>>();
    let chars = indexed.iter().map(|&(_, ch)| ch).collect::<Vec<_>>();
    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();

        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }

        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }

        match ch {
            '(' | '[' => stack.push((ch, indexed[index].0)),
            ')' | ']' => {
                stack.pop();
            }
            _ => {}
        }
        index += 1;
    }

    stack.pop()
}

pub(super) fn has_unmatched_open_brace(line: &str) -> bool {
    let chars = line.chars().collect::<Vec<_>>();
    let mut depth = 0usize;
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;

    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }

    depth > 0
}

// A middle line of a multiline block comment has no lexical marker of its own.
pub(super) fn reverse_scan_skips_block_comment(trimmed: &str, in_block_comment: &mut bool) -> bool {
    if *in_block_comment {
        if trimmed.contains("/*") {
            *in_block_comment = false;
        }
        return true;
    }
    let line = trimmed.trim_end();
    if let Some(body) = line.strip_suffix("*/")
        && !body.contains("/*")
    {
        *in_block_comment = true;
        return true;
    }
    false
}

pub(super) fn inline_brace_pair_range(line: &str) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    let mut depth = 0usize;
    let mut first_open = None;
    for token in tokenize(line) {
        let text = token_text(&token);
        match token {
            Token::Symbol('{') => {
                if depth == 0 {
                    first_open = Some(offset);
                }
                depth += 1;
            }
            Token::Symbol('}') if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    return first_open.map(|start| (start, offset + text.len()));
                }
            }
            _ => {}
        }
        offset += text.len();
    }
    None
}

pub(super) fn trailing_comment_split_limit(line: &str) -> usize {
    trailing_comment_start(line)
        .map(|index| line[..index].trim_end().len())
        .unwrap_or(line.len())
}

pub(super) fn line_comment_split_limit(line: &str) -> usize {
    line_comment_start(line)
        .map(|index| line[..index].trim_end().len())
        .unwrap_or(line.len())
}

pub(super) fn trailing_comment_start(line: &str) -> Option<usize> {
    if !line.contains("//") && !line.contains("/*") {
        return None;
    }
    trailing_comment_start_in_tokens(line, true)
}

fn trailing_comment_start_in_tokens(line: &str, inspect_preprocessor: bool) -> Option<usize> {
    let mut offset = 0usize;
    let mut trailing_start = None;
    for token in tokenize(line) {
        let text = token_text(&token);
        match token {
            Token::Comment(CommentKind::Line, _) => {
                return Some(trailing_start.unwrap_or(offset));
            }
            Token::Comment(CommentKind::Block, _) => {
                trailing_start.get_or_insert(offset);
            }
            Token::Whitespace(_) => {}
            Token::Preprocessor(value) if inspect_preprocessor => {
                let body = value.text.strip_prefix('#').unwrap_or(&value.text);
                trailing_start = trailing_comment_start_in_tokens(body, false)
                    .map(|index| offset + value.text.len() - body.len() + index);
            }
            _ => trailing_start = None,
        }
        offset += text.len();
    }
    trailing_start
}

fn line_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut block_comment = false;
    while index < bytes.len() {
        let ch = bytes[index];
        if block_comment {
            if ch == b'*' && bytes.get(index + 1) == Some(&b'/') {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == b'\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        match ch {
            b'"' | b'\'' => quote = Some(ch),
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                block_comment = true;
                index += 2;
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => return Some(index),
            _ => {}
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_whole_comment_lines() {
        for line in ["// line", " /* block", "* body", "*/"] {
            assert!(is_comment_line(line), "{line}");
        }
        for line in ["call(); // trailing", "value * other", ""] {
            assert!(!is_comment_line(line), "{line}");
        }
    }

    #[test]
    fn detects_comments_at_line_end_outside_literals() {
        for line in ["call(); // trailing", "call(); /* trailing */"] {
            assert!(line_ends_with_comment(line), "{line}");
        }
        for line in [
            "call(\"// not a comment\");",
            "call('\"');",
            "call(\"escaped \\\" // text\");",
        ] {
            assert!(!line_ends_with_comment(line), "{line}");
        }
    }

    #[test]
    fn trailing_comment_boundary_keeps_code_after_block_comments() {
        let interstitial = "switch /* comment */ (value) {";
        assert_eq!(
            trailing_comment_split_limit(interstitial),
            interstitial.len()
        );

        let trailing = "value /* first */ /* second */";
        assert_eq!(trailing_comment_split_limit(trailing), "value".len());

        let raw = "R\"(// not a comment)\" + value /* comment */";
        assert_eq!(
            trailing_comment_split_limit(raw),
            "R\"(// not a comment)\" + value".len()
        );
        let preprocessor = "#define VALUE 1'000 /* comment */";
        assert_eq!(
            trailing_comment_split_limit(preprocessor),
            "#define VALUE 1'000".len()
        );
    }
}
