//! Splits normalized source text into tokens.

use crate::formatter::constructs::assembly::AssemblyMacroLines;
use crate::formatter::syntax::language;
use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::formatter::text::line_scan::preprocessor_directive;
use crate::source::lex::{is_identifier_continue, is_identifier_start};
use std::borrow::Cow;

pub(crate) mod raw_strings;
mod text;

pub(crate) use text::TokenText;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Token {
    Word(TokenText),
    Number(TokenText),
    StringLiteral(TokenText),
    CharLiteral(TokenText),
    Comment(CommentKind, TokenText),
    Preprocessor(Box<PreprocessorToken>),
    RawLine(TokenText),
    Operator(TokenText),
    Symbol(char),
    Whitespace(TokenText),
    Newline,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CommentKind {
    Line,
    Block,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PreprocessorToken {
    pub(crate) text: String,
    pub(crate) opaque_literal_line_ranges: Vec<(usize, usize)>,
}

// The lexer walks the source by byte: every character it tells apart is
// ASCII, and no byte of a wider character equals one.

/// The character starting at byte `index`, ASCII without decoding.
fn char_at(source: &str, index: usize) -> Option<char> {
    let byte = *source.as_bytes().get(index)?;
    if byte.is_ascii() {
        Some(char::from(byte))
    } else {
        source[index..].chars().next()
    }
}

/// The byte of the line break ending the line that holds `index`.
fn line_end_from(source: &str, index: usize) -> usize {
    source.as_bytes()[index..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(source.len(), |offset| index + offset)
}

fn previous_char(source: &str, index: usize) -> Option<char> {
    source[..index].chars().next_back()
}

fn hash_after_statement_opens_preprocessor(source: &str, line_start: usize, hash: usize) -> bool {
    let line_end = line_end_from(source, hash);
    let line = &source[hash..line_end];
    let prefix = &source[line_start..hash];
    if line.contains_any_byte(b"{}")
        || line[1..].contains('#')
        || prefix.contains('#')
        || has_unclosed_grouping(prefix)
        || has_unclosed_grouping(line)
    {
        return false;
    }
    let Some(directive) = preprocessor_directive(line) else {
        return false;
    };
    if !is_known_hash_directive(directive) {
        return false;
    }
    prefix
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|ch| matches!(ch, '{' | '}' | ';'))
}

fn has_unclosed_grouping(text: &str) -> bool {
    let mut paren_depth = 0;
    let mut bracket_depth = 0;
    for byte in text.bytes() {
        match byte {
            b'(' => paren_depth += 1,
            b')' => {
                if paren_depth == 0 {
                    return true;
                }
                paren_depth -= 1;
            }
            b'[' => bracket_depth += 1,
            b']' => {
                if bracket_depth == 0 {
                    return true;
                }
                bracket_depth -= 1;
            }
            _ => {}
        }
    }
    paren_depth != 0 || bracket_depth != 0
}

fn unknown_hash_line_has_brace_code(line: &str) -> bool {
    let Some(directive) = preprocessor_directive(line) else {
        return false;
    };
    !is_known_hash_directive(directive) && line.contains_any_byte(b"{}")
}

fn is_known_hash_directive(directive: &str) -> bool {
    matches!(
        directive,
        "if" | "ifdef"
            | "ifndef"
            | "elif"
            | "elifdef"
            | "elifndef"
            | "else"
            | "endif"
            | "define"
            | "include"
            | "include_next"
            | "import"
            | "line"
            | "error"
            | "warning"
            | "pragma"
            | "undef"
            | "region"
            | "endregion"
    )
}

pub(crate) fn tokenize(source: &str) -> Vec<Token> {
    let shared = std::rc::Rc::new(source.to_owned());
    let text = |start: usize, end: usize| TokenText::slice(&shared, start, end);
    let bytes = source.as_bytes();
    // A token takes some four bytes of source on average.
    let mut tokens = Vec::with_capacity(source.len() / 4);
    let mut index = 0;
    let mut line_has_code = false;
    let mut line_start_index = 0usize;
    let mut assembly_macro_lines = AssemblyMacroLines::default();

    while index < bytes.len() {
        if index == line_start_index && line_may_be_raw(&source[index..], &assembly_macro_lines) {
            let line_end = line_end_from(source, index);
            let line = &source[index..line_end];
            let trimmed = line.trim_ascii_start();
            if is_full_line_conflict_marker(trimmed) {
                line_has_code = !trimmed.is_empty();
                tokens.push(Token::RawLine(text(index, line_end)));
                index = line_end;
                continue;
            } else if let Some(output) = assembly_macro_lines.take_raw_line(line) {
                tokens.push(Token::RawLine(output.into()));
                index = line_end;
                line_has_code = !line.is_empty();
                continue;
            }
        }
        if let Some((token, next_index)) = read_prefixed_literal(&shared, index) {
            tokens.push(token);
            index = next_index;
            line_has_code = true;
            continue;
        }
        let Some(ch) = char_at(source, index) else {
            break;
        };
        let next = bytes.get(index + 1).copied();
        match ch {
            '\n' => {
                tokens.push(Token::Newline);
                line_has_code = false;
                index += 1;
                line_start_index = index;
            }
            ch if ch.is_whitespace() => {
                let next_index = read_while(source, index, |ch| ch.is_whitespace() && ch != '\n');
                tokens.push(Token::Whitespace(text(index, next_index)));
                index = next_index;
            }
            '#' if !line_has_code
                || hash_after_statement_opens_preprocessor(source, line_start_index, index) =>
            {
                let line = &source[index..line_end_from(source, index)];
                if !line_has_code && unknown_hash_line_has_brace_code(line) {
                    tokens.push(Token::Symbol(ch));
                    index += 1;
                    line_has_code = true;
                } else {
                    assembly_macro_lines.observe_preprocessor();
                    let (preprocessor, next_index) = read_preprocessor(source, index);
                    tokens.push(Token::Preprocessor(Box::new(preprocessor)));
                    index = next_index;
                    line_has_code = true;
                }
            }
            '/' if next == Some(b'/') => {
                let next_index = read_line_comment(source, index);
                tokens.push(Token::Comment(CommentKind::Line, text(index, next_index)));
                index = next_index;
                line_has_code = true;
            }
            '/' if next == Some(b'*') => {
                let next_index = read_block_comment(source, index);
                tokens.push(Token::Comment(CommentKind::Block, text(index, next_index)));
                index = next_index;
                line_has_code = true;
            }
            '"' => {
                let (next_index, _) = read_quoted(source, index, b'"');
                tokens.push(Token::StringLiteral(text(index, next_index)));
                index = next_index;
                line_has_code = true;
            }
            '\'' => {
                let (next_index, _) = read_quoted(source, index, b'\'');
                tokens.push(Token::CharLiteral(text(index, next_index)));
                index = next_index;
                line_has_code = true;
            }
            '.' if next.is_some_and(|byte| byte.is_ascii_digit()) => {
                let next_index = read_number(source, index);
                tokens.push(Token::Number(text(index, next_index)));
                index = next_index;
                line_has_code = true;
            }
            ch if is_identifier_start(ch) => {
                let next_index = read_while(source, index, is_identifier_continue);
                tokens.push(Token::Word(text(index, next_index)));
                index = next_index;
                line_has_code = true;
            }
            ch if ch.is_ascii_digit() => {
                let next_index = read_number(source, index);
                tokens.push(Token::Number(text(index, next_index)));
                index = next_index;
                line_has_code = true;
            }
            _ => {
                if let Some(operator) = language::match_operator(source, index) {
                    tokens.push(Token::Operator(text(index, index + operator.len())));
                    index += operator.len();
                } else {
                    tokens.push(Token::Symbol(ch));
                    index += ch.len_utf8();
                }
                line_has_code = true;
            }
        }
    }

    tokens.shrink_to_fit();
    tokens
}

/// Whether the line starting `rest` may be a conflict marker or an
/// assembly macro line, from its first character past leading whitespace.
fn line_may_be_raw(rest: &str, assembly_macro_lines: &AssemblyMacroLines) -> bool {
    let first = rest
        .chars()
        .take_while(|&ch| ch != '\n')
        .find(|ch| !ch.is_whitespace());
    matches!(first, Some('<' | '=' | '>' | '|'))
        || assembly_macro_lines.may_take_line_starting_with(first)
}

fn is_full_line_conflict_marker(trimmed: &str) -> bool {
    trimmed.starts_with("<<<<<<<")
        || trimmed == "======="
        || trimmed.starts_with(">>>>>>>")
        || trimmed.starts_with("|||||||")
}

/// A directive from its `#` on: its text runs to the line break no `\`
/// continues, past breaks inside a block comment, as written.
fn read_preprocessor(source: &str, start: usize) -> (PreprocessorToken, usize) {
    let bytes = source.as_bytes();
    let mut index = start;
    let mut in_block_comment = false;
    let mut in_line_comment = false;
    let mut quote = None;
    let mut quote_start_line = None;
    let mut escaped = false;
    let mut line_start = start;
    let mut line_index = 0usize;
    let mut preserve_trailing_whitespace = false;
    let mut opaque_literal_line_ranges = Vec::new();
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\n' {
            let continued_line = source[line_start..index].trim_ascii_end().ends_with('\\')
                && !following_physical_line_is_blank(bytes, index + 1);
            if continued_line || in_block_comment {
                line_index += 1;
                index += 1;
                line_start = index;
                escaped = false;
                continue;
            }
            break;
        }
        if in_line_comment {
            index += 1;
            continue;
        }
        if in_block_comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_byte) = quote {
            let len = char_at(source, index).map_or(1, char::len_utf8);
            index += len;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote_byte {
                quote = None;
                quote_start_line = None;
            }
            continue;
        }
        let starts_literal = index == start
            || previous_char(source, index).is_none_or(|ch| !is_identifier_continue(ch));
        if starts_literal && let Some(prefix_len) = raw_string_prefix_len(source, index) {
            let (next_index, terminated) = read_raw_string(source, index, prefix_len);
            preserve_trailing_whitespace |= !terminated;
            let first_raw_line = line_index;
            for (offset, literal_byte) in bytes[index..next_index].iter().enumerate() {
                if *literal_byte == b'\n' {
                    line_index += 1;
                    line_start = index + offset + 1;
                }
            }
            if first_raw_line != line_index || !terminated {
                opaque_literal_line_ranges.push((first_raw_line, line_index));
            }
            index = next_index;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            quote = Some(byte);
            quote_start_line = Some(line_index);
            escaped = false;
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            in_line_comment = true;
            index += 2;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        index += char_at(source, index).map_or(1, char::len_utf8);
    }
    if let Some(start_line) = quote_start_line {
        preserve_trailing_whitespace = true;
        opaque_literal_line_ranges.push((start_line, line_index));
    }
    let output = &source[start..index];
    let text = if preserve_trailing_whitespace {
        output
    } else {
        output.trim_ascii_end()
    };
    (
        PreprocessorToken {
            text: text.to_string(),
            opaque_literal_line_ranges,
        },
        index,
    )
}

fn following_physical_line_is_blank(bytes: &[u8], start: usize) -> bool {
    bytes[start.min(bytes.len())..]
        .iter()
        .take_while(|&&byte| byte != b'\n')
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
}

/// The end of a line comment: the line break no `\` before it continues.
fn read_line_comment(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut line_start = start;
    let mut index = start;
    loop {
        let end = line_end_from(source, index);
        if end < bytes.len() && source[line_start..end].ends_with('\\') {
            index = end + 1;
            line_start = index;
            continue;
        }
        return end;
    }
}

/// The end of a block comment: past its `*/`, or the source's end.
fn read_block_comment(source: &str, start: usize) -> usize {
    source[start + 2..]
        .find("*/")
        .map_or(source.len(), |offset| start + 2 + offset + 2)
}

fn read_prefixed_literal(shared: &std::rc::Rc<String>, start: usize) -> Option<(Token, usize)> {
    let source: &str = shared;
    // Every prefix starts with one of these.
    if !matches!(
        source.as_bytes().get(start),
        Some(b'u' | b'U' | b'L' | b'R')
    ) {
        return None;
    }
    if let Some(prefix_len) = raw_string_prefix_len(source, start) {
        let (next_index, _) = read_raw_string(source, start, prefix_len);
        return Some((
            Token::StringLiteral(TokenText::slice(shared, start, next_index)),
            next_index,
        ));
    }

    for prefix in ["u8", "L", "u", "U"] {
        if !source[start..].starts_with(prefix) {
            continue;
        }
        let quote = match source.as_bytes().get(start + prefix.len()) {
            Some(b'"') => b'"',
            Some(b'\'') => b'\'',
            _ => continue,
        };
        let (next_index, _) = read_quoted(source, start + prefix.len(), quote);
        let text = TokenText::slice(shared, start, next_index);
        let token = if quote == b'"' {
            Token::StringLiteral(text)
        } else {
            Token::CharLiteral(text)
        };
        return Some((token, next_index));
    }

    None
}

fn raw_string_prefix_len(source: &str, start: usize) -> Option<usize> {
    let rest = &source[start..];
    ["u8R", "LR", "uR", "UR", "R"]
        .into_iter()
        .find(|prefix| rest.starts_with(prefix) && rest.as_bytes().get(prefix.len()) == Some(&b'"'))
        .map(str::len)
}

/// The end of a raw string literal and whether it closed; one whose
/// delimiter never opens reads as a plain literal.
fn read_raw_string(source: &str, start: usize, prefix_len: usize) -> (usize, bool) {
    let quote_index = start + prefix_len;
    let Some(open_offset) = source[quote_index + 1..].find(['(', '\n']) else {
        return read_quoted(source, quote_index, b'"');
    };
    let open_paren = quote_index + 1 + open_offset;
    if source.as_bytes()[open_paren] == b'\n' {
        return read_quoted(source, quote_index, b'"');
    }
    let delimiter = &source[quote_index + 1..open_paren];
    let closing = format!("){delimiter}\"");
    match source[open_paren + 1..].find(&closing) {
        Some(offset) => (open_paren + 1 + offset + closing.len(), true),
        None => (source.len(), false),
    }
}

/// The end of a quoted literal opening at `start`, and whether it closed: a
/// line break ends one, unless escaped.
fn read_quoted(source: &str, start: usize, quote: u8) -> (usize, bool) {
    let bytes = source.as_bytes();
    let mut index = start + 1;
    while let Some(offset) = bytes[index.min(bytes.len())..]
        .iter()
        .position(|&byte| byte == b'\\' || byte == quote || byte == b'\n')
    {
        index += offset;
        match bytes[index] {
            b'\n' => return (index, false),
            b'\\' => {
                index = (index + 1 + char_at(source, index + 1).map_or(0, char::len_utf8))
                    .min(bytes.len())
            }
            _ => return (index + 1, true),
        }
    }
    (bytes.len(), false)
}

fn read_number(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut index = start;
    while let Some(&byte) = bytes.get(index) {
        let digit_separator = byte == b'\''
            && index > 0
            && bytes[index - 1].is_ascii_hexdigit()
            && bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit);
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'.' | b'_')
            || digit_separator
            || matches!(byte, b'+' | b'-')
                && index > start
                && matches!(bytes[index - 1], b'e' | b'E' | b'p' | b'P')
        {
            index += 1;
        } else {
            break;
        }
    }
    index
}

/// The end of the run of characters from `start` that `predicate` holds for.
fn read_while(source: &str, start: usize, predicate: impl Fn(char) -> bool) -> usize {
    let mut index = start;
    while let Some(ch) = char_at(source, index).filter(|&ch| predicate(ch)) {
        index += ch.len_utf8();
    }
    index
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TokenLine {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct TokenLineCursor<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl<'a> TokenLineCursor<'a> {
    pub(crate) fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    pub(crate) fn next_line(&mut self) -> Option<TokenLine> {
        if self.position >= self.tokens.len() {
            return None;
        }

        let start = self.position;
        while self.position < self.tokens.len() {
            let is_newline = matches!(self.tokens[self.position], Token::Newline);
            self.position += 1;
            if is_newline {
                break;
            }
        }

        Some(TokenLine {
            start,
            end: self.position,
        })
    }
}

pub(crate) fn matching_close_paren_index(tokens: &[Token], open_paren: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open_paren) {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn previous_non_layout_token_index(tokens: &[Token], before: usize) -> Option<usize> {
    (0..before)
        .rev()
        .find(|index| !matches!(tokens[*index], Token::Whitespace(_) | Token::Newline))
}

pub(crate) fn next_non_layout_token_index(tokens: &[Token], start: usize) -> Option<usize> {
    (start..tokens.len())
        .find(|index| !matches!(tokens[*index], Token::Whitespace(_) | Token::Newline))
}

pub(crate) fn next_non_whitespace(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    (start..end).find(|index| !matches!(tokens[*index], Token::Whitespace(_)))
}

pub(crate) fn token_char_len(token: &Token) -> usize {
    match token {
        Token::Word(value)
        | Token::Number(value)
        | Token::StringLiteral(value)
        | Token::CharLiteral(value)
        | Token::RawLine(value)
        | Token::Operator(value)
        | Token::Whitespace(value)
        | Token::Comment(_, value) => value.chars().count(),
        Token::Preprocessor(value) => value.text.chars().count(),
        Token::Symbol(_) | Token::Newline => 1,
    }
}

/// The first token of `tokens` that holds more than whitespace: the one the
/// trimmed text of a line of them starts with.
pub(crate) fn first_visible_token(tokens: &[Token]) -> Option<&Token> {
    tokens
        .iter()
        .find(|token| !token_text(token).chars().all(char::is_whitespace))
}

/// The last token of `tokens` that holds more than whitespace.
pub(crate) fn last_visible_token(tokens: &[Token]) -> Option<&Token> {
    tokens
        .iter()
        .rev()
        .find(|token| !token_text(token).chars().all(char::is_whitespace))
}

pub(crate) fn token_text(token: &Token) -> Cow<'_, str> {
    // Each ASCII character's own text, for symbols.
    const ASCII: &str = "\0\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x0b\x0c\r\x0e\x0f\
        \x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f \
        !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\x7f";
    match token {
        Token::Word(value)
        | Token::Number(value)
        | Token::StringLiteral(value)
        | Token::CharLiteral(value)
        | Token::RawLine(value)
        | Token::Operator(value)
        | Token::Whitespace(value)
        | Token::Comment(_, value) => Cow::Borrowed(value),
        Token::Preprocessor(value) => Cow::Borrowed(&value.text),
        Token::Symbol(value) if value.is_ascii() => {
            let index = *value as usize;
            Cow::Borrowed(&ASCII[index..=index])
        }
        Token::Symbol(value) => Cow::Owned(value.to_string()),
        Token::Newline => Cow::Borrowed("\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::{TokenLine, TokenLineCursor, tokenize};

    #[test]
    fn line_cursor_groups_physical_lines() {
        let tokens = tokenize("int a;\nint b;");
        let mut cursor = TokenLineCursor::new(&tokens);

        assert_eq!(cursor.next_line(), Some(TokenLine { start: 0, end: 5 }));
        assert_eq!(cursor.next_line(), Some(TokenLine { start: 5, end: 9 }));
        assert_eq!(cursor.next_line(), None);
    }
}
