use super::assembly::AssemblyMacroLines;
use super::language;
use crate::source::lex::{is_digit_separator, is_identifier_continue, is_identifier_start};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum Token {
    Word(String),
    Number(String),
    StringLiteral(String),
    CharLiteral(String),
    Comment(CommentKind, String),
    Preprocessor(PreprocessorToken),
    RawLine(String),
    Operator(String),
    Symbol(char),
    Whitespace(String),
    Newline,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum CommentKind {
    Line,
    Block,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct PreprocessorToken {
    pub(super) text: String,
    pub(super) opaque_literal_line_ranges: Vec<(usize, usize)>,
}

fn hash_after_statement_opens_preprocessor(chars: &[char], line_start: usize, hash: usize) -> bool {
    let line_end = chars[hash..]
        .iter()
        .position(|&ch| ch == '\n')
        .map_or(chars.len(), |offset| hash + offset);
    let line = chars[hash..line_end].iter().collect::<String>();
    let prefix = &chars[line_start..hash];
    if line.contains(['{', '}'])
        || line[1..].contains('#')
        || prefix.contains(&'#')
        || has_unclosed_grouping(prefix)
        || has_unclosed_grouping(&chars[hash..line_end])
    {
        return false;
    }
    let Some(directive) = hash_line_directive(&line) else {
        return false;
    };
    if !is_known_hash_directive(directive) {
        return false;
    }
    chars[line_start..hash]
        .iter()
        .rev()
        .find(|ch| !ch.is_whitespace())
        .is_some_and(|ch| matches!(ch, '{' | '}' | ';'))
}

fn has_unclosed_grouping(chars: &[char]) -> bool {
    let mut paren_depth = 0;
    let mut bracket_depth = 0;
    for ch in chars {
        match ch {
            '(' => paren_depth += 1,
            ')' => {
                if paren_depth == 0 {
                    return true;
                }
                paren_depth -= 1;
            }
            '[' => bracket_depth += 1,
            ']' => {
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
    let Some(directive) = hash_line_directive(line) else {
        return false;
    };
    !is_known_hash_directive(directive) && line.contains(['{', '}'])
}

fn hash_line_directive(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
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

pub(super) fn tokenize(source: &str) -> Vec<Token> {
    let chars = source.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line_has_code = false;
    let mut line_start_index = 0usize;
    let mut assembly_macro_lines = AssemblyMacroLines::default();

    while index < chars.len() {
        if index == line_start_index {
            let line_end = chars[index..]
                .iter()
                .position(|&ch| ch == '\n')
                .map_or(chars.len(), |offset| index + offset);
            let line = chars[index..line_end].iter().collect::<String>();
            let trimmed = line.trim_start();
            if is_full_line_conflict_marker(trimmed) {
                line_has_code = !trimmed.is_empty();
                tokens.push(Token::RawLine(line));
                index = line_end;
                continue;
            } else if let Some(output) = assembly_macro_lines.take_raw_line(&line) {
                tokens.push(Token::RawLine(output));
                index = line_end;
                line_has_code = !line.is_empty();
                continue;
            }
        }
        if let Some((token, next_index)) = read_prefixed_literal(&chars, index) {
            tokens.push(token);
            index = next_index;
            line_has_code = true;
            continue;
        }
        let ch = chars[index];
        match ch {
            '\n' => {
                tokens.push(Token::Newline);
                line_has_code = false;
                index += 1;
                line_start_index = index;
            }
            ch if ch.is_whitespace() => {
                let (whitespace, next_index) =
                    read_while(&chars, index, |ch| ch.is_whitespace() && ch != '\n');
                tokens.push(Token::Whitespace(whitespace));
                index = next_index;
            }
            '#' if !line_has_code
                || hash_after_statement_opens_preprocessor(&chars, line_start_index, index) =>
            {
                let line_end = chars[index..]
                    .iter()
                    .position(|&ch| ch == '\n')
                    .map_or(chars.len(), |offset| index + offset);
                let line = chars[index..line_end].iter().collect::<String>();
                if !line_has_code && unknown_hash_line_has_brace_code(&line) {
                    tokens.push(Token::Symbol(ch));
                    index += 1;
                    line_has_code = true;
                } else {
                    assembly_macro_lines.observe_preprocessor();
                    let (preprocessor, next_index) = read_preprocessor(&chars, index);
                    tokens.push(Token::Preprocessor(preprocessor));
                    index = next_index;
                    line_has_code = true;
                }
            }
            '/' if chars.get(index + 1) == Some(&'/') => {
                let (comment, next_index) = read_line_comment(&chars, index);
                tokens.push(Token::Comment(CommentKind::Line, comment));
                index = next_index;
                line_has_code = true;
            }
            '/' if chars.get(index + 1) == Some(&'*') => {
                let (comment, next_index) = read_block_comment(&chars, index);
                tokens.push(Token::Comment(CommentKind::Block, comment));
                index = next_index;
                line_has_code = true;
            }
            '"' => {
                let (literal, next_index, _) = read_quoted(&chars, index, '"');
                tokens.push(Token::StringLiteral(literal));
                index = next_index;
                line_has_code = true;
            }
            '\'' => {
                let (literal, next_index, _) = read_quoted(&chars, index, '\'');
                tokens.push(Token::CharLiteral(literal));
                index = next_index;
                line_has_code = true;
            }
            '.' if chars.get(index + 1).is_some_and(char::is_ascii_digit) => {
                let (number, next_index) = read_number(&chars, index);
                tokens.push(Token::Number(number));
                index = next_index;
                line_has_code = true;
            }
            ch if is_identifier_start(ch) => {
                let (word, next_index) = read_while(&chars, index, is_identifier_continue);
                tokens.push(Token::Word(word));
                index = next_index;
                line_has_code = true;
            }
            ch if ch.is_ascii_digit() => {
                let (number, next_index) = read_number(&chars, index);
                tokens.push(Token::Number(number));
                index = next_index;
                line_has_code = true;
            }
            _ => {
                if let Some(operator) = language::match_operator(&chars, index) {
                    tokens.push(Token::Operator(operator.to_string()));
                    index += operator.chars().count();
                } else {
                    tokens.push(Token::Symbol(ch));
                    index += 1;
                }
                line_has_code = true;
            }
        }
    }

    tokens
}

fn is_full_line_conflict_marker(trimmed: &str) -> bool {
    trimmed.starts_with("<<<<<<<")
        || trimmed == "======="
        || trimmed.starts_with(">>>>>>>")
        || trimmed.starts_with("|||||||")
}

fn read_preprocessor(chars: &[char], start: usize) -> (PreprocessorToken, usize) {
    let mut index = start;
    let mut output = String::new();
    let mut in_block_comment = false;
    let mut in_line_comment = false;
    let mut quote = None;
    let mut quote_start_line = None;
    let mut escaped = false;
    let mut line_start = 0usize;
    let mut line_index = 0usize;
    let mut preserve_trailing_whitespace = false;
    let mut opaque_literal_line_ranges = Vec::new();
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' {
            let current_line = &output[line_start..];
            let continued_line = current_line.trim_end().ends_with('\\')
                && !following_physical_line_is_blank(chars, index + 1);
            if continued_line || in_block_comment {
                output.push('\n');
                line_index += 1;
                index += 1;
                line_start = output.len();
                escaped = false;
                continue;
            }
            break;
        }
        if in_line_comment {
            output.push(ch);
            index += 1;
            continue;
        }
        if in_block_comment {
            if ch == '*' && chars.get(index + 1) == Some(&'/') {
                in_block_comment = false;
                output.push('*');
                output.push('/');
                index += 2;
            } else {
                output.push(ch);
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            output.push(ch);
            index += 1;
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
                quote_start_line = None;
            }
            continue;
        }
        let starts_literal = index == start
            || chars
                .get(index.wrapping_sub(1))
                .is_none_or(|ch| !is_identifier_continue(*ch));
        if starts_literal && let Some(prefix_len) = raw_string_prefix_len(chars, index) {
            let (_, next_index, terminated) = read_raw_string(chars, index, prefix_len);
            preserve_trailing_whitespace |= !terminated;
            let first_raw_line = line_index;
            for literal_ch in &chars[index..next_index] {
                output.push(*literal_ch);
                if *literal_ch == '\n' {
                    line_index += 1;
                    line_start = output.len();
                }
            }
            if first_raw_line != line_index || !terminated {
                opaque_literal_line_ranges.push((first_raw_line, line_index));
            }
            index = next_index;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            quote_start_line = Some(line_index);
            escaped = false;
            output.push(ch);
            index += 1;
            continue;
        }
        if ch == '/' && chars.get(index + 1) == Some(&'/') {
            in_line_comment = true;
            output.push('/');
            output.push('/');
            index += 2;
            continue;
        }
        if ch == '/' && chars.get(index + 1) == Some(&'*') {
            in_block_comment = true;
            output.push('/');
            output.push('*');
            index += 2;
            continue;
        }
        output.push(ch);
        index += 1;
    }
    if let Some(start_line) = quote_start_line {
        preserve_trailing_whitespace = true;
        opaque_literal_line_ranges.push((start_line, line_index));
    }
    let text = if preserve_trailing_whitespace {
        output
    } else {
        output.trim_end().to_string()
    };
    (
        PreprocessorToken {
            text,
            opaque_literal_line_ranges,
        },
        index,
    )
}

fn following_physical_line_is_blank(chars: &[char], start: usize) -> bool {
    let mut index = start;
    while index < chars.len() && chars[index] != '\n' {
        if !matches!(chars[index], ' ' | '\t' | '\r') {
            return false;
        }
        index += 1;
    }
    true
}

fn read_line_comment(chars: &[char], start: usize) -> (String, usize) {
    let mut index = start;
    let mut output = String::new();
    let mut line_start = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' {
            if output[line_start..].ends_with('\\') {
                output.push(ch);
                index += 1;
                line_start = output.len();
                continue;
            }
            break;
        }
        output.push(ch);
        index += 1;
    }
    (output, index)
}

fn read_block_comment(chars: &[char], start: usize) -> (String, usize) {
    let mut index = start + 2;
    let mut output = String::from("/*");
    while index < chars.len() {
        let ch = chars[index];
        output.push(ch);
        index += 1;
        if ch == '*' && chars.get(index) == Some(&'/') {
            output.push('/');
            index += 1;
            break;
        }
    }
    (output, index)
}

fn read_prefixed_literal(chars: &[char], start: usize) -> Option<(Token, usize)> {
    if let Some(prefix_len) = raw_string_prefix_len(chars, start) {
        let (literal, next_index, _) = read_raw_string(chars, start, prefix_len);
        return Some((Token::StringLiteral(literal), next_index));
    }

    for prefix in ["u8", "L", "u", "U"] {
        if !chars_match(chars, start, prefix) {
            continue;
        }
        match chars.get(start + prefix.len()).copied() {
            Some('"') => {
                let (quoted, next_index, _) = read_quoted(chars, start + prefix.len(), '"');
                return Some((
                    Token::StringLiteral(format!("{prefix}{quoted}")),
                    next_index,
                ));
            }
            Some('\'') => {
                let (quoted, next_index, _) = read_quoted(chars, start + prefix.len(), '\'');
                return Some((Token::CharLiteral(format!("{prefix}{quoted}")), next_index));
            }
            _ => {}
        }
    }

    None
}

fn raw_string_prefix_len(chars: &[char], start: usize) -> Option<usize> {
    ["u8R", "LR", "uR", "UR", "R"]
        .into_iter()
        .find(|prefix| {
            chars_match(chars, start, prefix) && chars.get(start + prefix.len()) == Some(&'"')
        })
        .map(str::len)
}

fn chars_match(chars: &[char], start: usize, text: &str) -> bool {
    text.chars()
        .enumerate()
        .all(|(offset, ch)| chars.get(start + offset) == Some(&ch))
}

fn read_raw_string(chars: &[char], start: usize, prefix_len: usize) -> (String, usize, bool) {
    let quote_index = start + prefix_len;
    let mut open_paren = quote_index + 1;
    while open_paren < chars.len() && chars[open_paren] != '(' {
        if chars[open_paren] == '\n' {
            return read_prefixed_quoted_fallback(chars, start, prefix_len);
        }
        open_paren += 1;
    }
    if open_paren >= chars.len() {
        return read_prefixed_quoted_fallback(chars, start, prefix_len);
    }

    let delimiter = chars[quote_index + 1..open_paren]
        .iter()
        .collect::<Vec<_>>();
    let mut index = open_paren + 1;
    while index < chars.len() {
        if chars[index] == ')'
            && delimiter
                .iter()
                .enumerate()
                .all(|(offset, ch)| chars.get(index + 1 + offset) == Some(ch))
            && chars.get(index + 1 + delimiter.len()) == Some(&'"')
        {
            let end = index + 2 + delimiter.len();
            return (chars[start..end].iter().collect(), end, true);
        }
        index += 1;
    }

    (chars[start..].iter().collect(), chars.len(), false)
}

fn read_prefixed_quoted_fallback(
    chars: &[char],
    start: usize,
    prefix_len: usize,
) -> (String, usize, bool) {
    let (quoted, next_index, terminated) = read_quoted(chars, start + prefix_len, '"');
    let prefix = chars[start..start + prefix_len].iter().collect::<String>();
    (format!("{prefix}{quoted}"), next_index, terminated)
}

fn read_quoted(chars: &[char], start: usize, quote: char) -> (String, usize, bool) {
    let mut index = start;
    let mut output = String::new();
    let mut escaped = false;
    let mut first = true;
    let mut terminated = false;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' && !escaped {
            break;
        }
        output.push(ch);
        index += 1;
        if first {
            first = false;
        } else if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            terminated = true;
            break;
        }
    }
    (output, index, terminated)
}

fn read_number(chars: &[char], start: usize) -> (String, usize) {
    let mut index = start;
    let mut output = String::new();
    while let Some(&ch) = chars.get(index) {
        if ch.is_ascii_alphanumeric()
            || matches!(ch, '.' | '_')
            || is_digit_separator(chars, index)
            || matches!(ch, '+' | '-')
                && output
                    .chars()
                    .last()
                    .is_some_and(|previous| matches!(previous, 'e' | 'E' | 'p' | 'P'))
        {
            output.push(ch);
            index += 1;
        } else {
            break;
        }
    }
    (output, index)
}

fn read_while(chars: &[char], start: usize, predicate: impl Fn(char) -> bool) -> (String, usize) {
    let mut index = start;
    let mut output = String::new();
    while let Some(&ch) = chars.get(index) {
        if !predicate(ch) {
            break;
        }
        output.push(ch);
        index += 1;
    }
    (output, index)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct TokenLine {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug, Clone)]
pub(super) struct TokenLineCursor<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl<'a> TokenLineCursor<'a> {
    pub(super) fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    pub(super) fn next_line(&mut self) -> Option<TokenLine> {
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

pub(super) fn matching_close_paren_index(tokens: &[Token], open_paren: usize) -> Option<usize> {
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

pub(super) fn next_non_layout_token_index(tokens: &[Token], start: usize) -> Option<usize> {
    (start..tokens.len())
        .find(|index| !matches!(tokens[*index], Token::Whitespace(_) | Token::Newline))
}

pub(super) fn next_non_whitespace(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    (start..end).find(|index| !matches!(tokens[*index], Token::Whitespace(_)))
}

pub(super) fn token_char_len(token: &Token) -> usize {
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

pub(super) fn token_text(token: &Token) -> String {
    match token {
        Token::Word(value)
        | Token::Number(value)
        | Token::StringLiteral(value)
        | Token::CharLiteral(value)
        | Token::RawLine(value)
        | Token::Operator(value)
        | Token::Whitespace(value)
        | Token::Comment(_, value) => value.clone(),
        Token::Preprocessor(value) => value.text.clone(),
        Token::Symbol(value) => value.to_string(),
        Token::Newline => "\n".to_string(),
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
