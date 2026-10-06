use crate::formatter::lexer::{CommentKind, Token, line_tokens_hold_comment, token_text, tokenize};
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::is_digit_separator;

/// The index of the first `byte` in `bytes`, read eight bytes at a time.
pub(crate) fn find_byte(bytes: &[u8], byte: u8) -> Option<usize> {
    const ONES: u64 = u64::from_le_bytes([0x01; 8]);
    const HIGHS: u64 = u64::from_le_bytes([0x80; 8]);
    let pattern = ONES * u64::from(byte);
    let mut chunks = bytes.chunks_exact(8);
    let mut index = 0;
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().expect("eight bytes")) ^ pattern;
        // The lowest high bit set marks the first zero byte: a borrow only
        // runs past a zero.
        let zeros = word.wrapping_sub(ONES) & !word & HIGHS;
        if zeros != 0 {
            return Some(index + zeros.trailing_zeros() as usize / 8);
        }
        index += 8;
    }
    chunks
        .remainder()
        .iter()
        .position(|&found| found == byte)
        .map(|offset| index + offset)
}

/// Byte-set membership for ASCII sets: one pass over the bytes, where a
/// `char` array pattern decodes every character.
pub(crate) trait ContainsAnyByte {
    fn contains_any_byte(&self, set: &[u8]) -> bool;

    /// Whether the text holds `needle`, looked for from the first byte of
    /// `needle` on; quick when that byte is rare.
    fn contains_from_first_byte(&self, needle: &str) -> bool;

    /// `str::find` of `needle`, looked for from the first byte of `needle`
    /// on; quick when that byte is rare.
    fn find_from_first_byte(&self, needle: &str) -> Option<usize>;

    /// Whether the text starts with one of the ASCII bytes of `set`.
    fn starts_with_any(&self, set: &[u8]) -> bool;

    /// Whether the text ends with one of the ASCII bytes of `set`.
    fn ends_with_any(&self, set: &[u8]) -> bool;
}

impl ContainsAnyByte for str {
    fn starts_with_any(&self, set: &[u8]) -> bool {
        debug_assert!(set.is_ascii());
        self.as_bytes()
            .first()
            .is_some_and(|byte| set.contains(byte))
    }

    fn ends_with_any(&self, set: &[u8]) -> bool {
        debug_assert!(set.is_ascii());
        self.as_bytes()
            .last()
            .is_some_and(|byte| set.contains(byte))
    }

    fn contains_from_first_byte(&self, needle: &str) -> bool {
        self.find_from_first_byte(needle).is_some()
    }

    fn find_from_first_byte(&self, needle: &str) -> Option<usize> {
        match needle.as_bytes().first() {
            Some(&first) => {
                let at = find_byte(self.as_bytes(), first)?;
                self[at..].find(needle).map(|offset| at + offset)
            }
            None => Some(0),
        }
    }

    fn contains_any_byte(&self, set: &[u8]) -> bool {
        debug_assert!(set.is_ascii());
        let bytes = self.as_bytes();
        if let [first, ..] = *set
            && set.len() <= 4
        {
            // Plain compares over whole chunks let the compiler use vector
            // instructions.
            let mut wanted = [first; 4];
            wanted[..set.len()].copy_from_slice(set);
            let hit = |byte: u8| wanted.iter().fold(false, |hit, &want| hit | (byte == want));
            let mut chunks = bytes.chunks_exact(32);
            for chunk in &mut chunks {
                if chunk.iter().fold(false, |found, &byte| found | hit(byte)) {
                    return true;
                }
            }
            return chunks.remainder().iter().any(|&byte| hit(byte));
        }
        let mask = set.iter().fold(0u128, |mask, &byte| mask | 1 << byte);
        bytes
            .iter()
            .any(|&byte| byte < 128 && mask >> byte & 1 != 0)
    }
}

/// The text after the last `;`, `{` or `}`.
pub(crate) fn statement_tail(text: &str) -> &str {
    text.as_bytes()
        .iter()
        .rposition(|byte| matches!(byte, b';' | b'{' | b'}'))
        .map_or(text, |index| &text[index + 1..])
}

pub(crate) fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trimmed_start();
    // A block comment row leads with a bare `*`; code may lead with `*p`.
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed
            .strip_prefix('*')
            .is_some_and(|rest| rest.is_empty() || rest.starts_with_any(b" \t*/"))
}

pub(crate) fn is_comment_only_line(line: &str) -> bool {
    line.starts_with("//")
        || line.starts_with("/*")
        || line.starts_with("*/")
        || line == "*"
        || line.starts_with("* ")
        || line.starts_with("*\t")
}

pub(crate) fn has_unclosed_delimiter_after(text: &str, open: &str, close: &str) -> bool {
    let last = |pattern: &str| match pattern.as_bytes() {
        &[byte] => text.rfind(char::from(byte)),
        _ => text.rfind(pattern),
    };
    last(open)
        .is_some_and(|open_index| last(close).is_none_or(|close_index| close_index < open_index))
}

pub(crate) fn trailing_matching_parens(line: &str) -> Option<(usize, usize)> {
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

pub(crate) fn has_top_level_comma_in_text(text: &str) -> bool {
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

pub(crate) fn line_ends_with_comment(line: &str) -> bool {
    let trimmed = line.trimmed_end();
    if trimmed.ends_with("*/") {
        return true;
    }
    // Only a `//` starts a comment that ends the line.
    if !trimmed.as_bytes().contains(&b'/') {
        return false;
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

pub(crate) fn find_outside_quotes(line: &str, needle: &str) -> Option<usize> {
    if !line.contains(needle) {
        return None;
    }
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

pub(crate) fn unmatched_open_paren_column(line: &str) -> Option<usize> {
    unmatched_open_paren_columns(line)
        .into_iter()
        .rev()
        .find(|&column| line[column + 1..].chars().any(|ch| !ch.is_whitespace()))
}

pub(crate) fn unmatched_open_bracket_column(line: &str) -> Option<usize> {
    unmatched_open_paren_columns(line)
        .into_iter()
        .rev()
        .find(|&column| line[column..].starts_with('['))
}

/// Whether `line` starts inside a block comment it closes: a row that a
/// comment opened on an earlier line.
fn continues_block_comment(line: &str) -> bool {
    find_comment_close(line).is_some_and(|close| {
        let before = &line[..close];
        !before.contains("/*") && !before.contains('"')
    })
}

pub(crate) fn line_paren_imbalance(line: &str) -> (usize, Vec<usize>) {
    scan_paren_imbalance(line)
}

/// Unmatched `)` and `]` of `line`, and the byte columns of its unmatched
/// `(` and `[`, outside literals and comments.
fn scan_paren_imbalance(line: &str) -> (usize, Vec<usize>) {
    // Every byte that matters is ASCII, and no byte of a wider character
    // equals one.
    const SCANNED: [bool; 256] = {
        let mut scanned = [false; 256];
        let bytes = b"/\"'()[]";
        let mut index = 0;
        while index < bytes.len() {
            scanned[bytes[index] as usize] = true;
            index += 1;
        }
        scanned
    };
    let bytes = line.as_bytes();
    if !line.contains_any_byte(b"()[]") {
        return (0, Vec::new());
    }
    let mut opens = OpenColumns::default();
    let mut unmatched_closes = 0usize;
    let mut index = match continues_block_comment(line) {
        true => find_comment_close(line).map_or(bytes.len(), |close| close + 2),
        false => 0,
    };
    while let Some(offset) = bytes[index.min(bytes.len())..]
        .iter()
        .position(|&byte| SCANNED[usize::from(byte)])
    {
        index += offset;
        match bytes[index] {
            b'/' if bytes.get(index + 1) == Some(&b'/') => break,
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = find_comment_close(&line[index + 2..])
                    .map_or(bytes.len(), |close| index + 2 + close + 2);
                continue;
            }
            b'"' => {
                index = skip_quoted(bytes, index + 1, b'"');
                continue;
            }
            b'\'' if !is_byte_digit_separator(bytes, index) => {
                index = skip_quoted(bytes, index + 1, b'\'');
                continue;
            }
            b'(' | b'[' => opens.push(index),
            b')' | b']' if opens.pop().is_none() => unmatched_closes += 1,
            _ => {}
        }
        index += 1;
    }
    (unmatched_closes, opens.into_vec())
}

/// Index after the `quote` that closes a literal whose text starts at
/// `from`, or the end of `bytes` when it stays open.
fn skip_quoted(bytes: &[u8], from: usize, quote: u8) -> usize {
    let mut index = from;
    while let Some(offset) = bytes[index.min(bytes.len())..]
        .iter()
        .position(|&byte| byte == b'\\' || byte == quote)
    {
        index += offset;
        if bytes[index] == quote {
            return index + 1;
        }
        index += 2;
    }
    bytes.len()
}

/// A stack of open paren columns that holds a few in place before it
/// allocates.
#[derive(Default)]
struct OpenColumns {
    near: [usize; 8],
    depth: usize,
    deep: Vec<usize>,
}

impl OpenColumns {
    fn push(&mut self, column: usize) {
        match self.near.get_mut(self.depth) {
            Some(slot) => *slot = column,
            None => self.deep.push(column),
        }
        self.depth += 1;
    }

    fn pop(&mut self) -> Option<usize> {
        self.depth = self.depth.checked_sub(1)?;
        match self.near.get(self.depth) {
            Some(&column) => Some(column),
            None => self.deep.pop(),
        }
    }

    fn into_vec(self) -> Vec<usize> {
        if self.depth == 0 {
            return Vec::new();
        }
        let mut columns = self.near[..self.depth.min(self.near.len())].to_vec();
        columns.extend(self.deep);
        columns
    }
}

/// Whether the `'` at byte `index` separates digits of a number.
fn is_byte_digit_separator(bytes: &[u8], index: usize) -> bool {
    index > 0
        && bytes[index - 1].is_ascii_hexdigit()
        && bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit)
}

/// Returns the brace imbalance of a single line as `(unmatched_closes, unmatched_opens)`,
/// ignoring braces inside strings and comments. A `}` without a matching `{` earlier on the
/// same line counts as an unmatched close; a `{` left open at the end counts as an open.
pub(crate) fn line_brace_imbalance(line: &str) -> (usize, usize) {
    if !line.contains_any_byte(b"{}") {
        return (0, 0);
    }
    // Every byte that matters is ASCII, and no byte of a wider character
    // equals one.
    let bytes = line.as_bytes();
    let mut open_depth = 0usize;
    let mut unmatched_closes = 0usize;
    let mut index = 0;
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    let mut in_block_comment = false;

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
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == open {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && next == Some(b'/') {
            break;
        }
        if byte == b'/' && next == Some(b'*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        let digit_separator = byte == b'\''
            && index > 0
            && bytes[index - 1].is_ascii_hexdigit()
            && next.is_some_and(|next| next.is_ascii_hexdigit());
        if byte == b'"' || (byte == b'\'' && !digit_separator) {
            quote = Some(byte);
            index += 1;
            continue;
        }
        match byte {
            b'{' => open_depth += 1,
            b'}' if open_depth > 0 => open_depth -= 1,
            b'}' => unmatched_closes += 1,
            _ => {}
        }
        index += 1;
    }
    (unmatched_closes, open_depth)
}

/// True when the line has a `{` or `}` outside strings and comments.
pub(crate) fn line_has_brace(line: &str) -> bool {
    if !line.bytes().any(|byte| matches!(byte, b'{' | b'}')) {
        return false;
    }
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
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
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

pub(crate) fn unmatched_open_paren_columns(line: &str) -> Vec<usize> {
    scan_paren_imbalance(line).1
}

pub(crate) fn last_unmatched_open_delimiter(line: &str) -> Option<(char, usize)> {
    let mut scan = DelimiterScan::default();
    scan.advance(line.as_bytes(), line.len());
    scan.last_open()
}

/// A scan for the `(` and `[` a line leaves open, outside literals and
/// comments, that goes on as the line grows. Every byte that matters is
/// ASCII, and no byte of a wider character equals one.
#[derive(Debug, Clone, Default)]
pub(crate) struct DelimiterScan {
    scanned: usize,
    open: Vec<(char, usize)>,
    quote: Option<char>,
    escaped: bool,
    in_block_comment: bool,
    /// A line comment ends the code.
    ended: bool,
}

impl DelimiterScan {
    /// Starts the scan afresh, keeping its buffer.
    pub(crate) fn reset(&mut self) {
        let mut open = std::mem::take(&mut self.open);
        open.clear();
        *self = Self {
            open,
            ..Self::default()
        };
    }

    pub(crate) fn scanned(&self) -> usize {
        self.scanned
    }

    /// Reads `bytes` on from where the scan stands, through the byte before
    /// `end` and any byte that byte pairs with.
    pub(crate) fn advance(&mut self, bytes: &[u8], end: usize) {
        while !self.ended && self.scanned < end {
            let index = self.scanned;
            let byte = bytes[index];
            let next = bytes.get(index + 1).copied();
            if self.in_block_comment {
                if byte == b'*' && next == Some(b'/') {
                    self.in_block_comment = false;
                    self.scanned += 2;
                } else {
                    self.scanned += 1;
                }
                continue;
            }
            if self.quote.is_some() {
                advance_quoted_literal(char::from(byte), &mut self.quote, &mut self.escaped);
                self.scanned += 1;
                continue;
            }
            if byte == b'/' && next == Some(b'/') {
                self.ended = true;
                break;
            }
            if byte == b'/' && next == Some(b'*') {
                self.in_block_comment = true;
                self.scanned += 2;
                continue;
            }
            let digit_separator = byte == b'\''
                && index > 0
                && bytes[index - 1].is_ascii_hexdigit()
                && next.is_some_and(|next| next.is_ascii_hexdigit());
            if byte == b'"' || (byte == b'\'' && !digit_separator) {
                self.quote = Some(char::from(byte));
                self.scanned += 1;
                continue;
            }
            match byte {
                b'(' | b'[' => self.open.push((char::from(byte), index)),
                b')' | b']' => {
                    self.open.pop();
                }
                _ => {}
            }
            self.scanned += 1;
        }
    }

    /// The last `(` or `[` left open, with its byte offset.
    pub(crate) fn last_open(&self) -> Option<(char, usize)> {
        self.open.last().copied()
    }
}

/// Byte offset of the first element after the last unmatched `{` when the
/// line continues past it.
pub(crate) fn unmatched_open_brace_content_offset(line: &str) -> Option<usize> {
    let mut stack: Vec<usize> = Vec::new();
    let mut chars = line.char_indices().peekable();
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;
    while let Some((offset, ch)) = chars.next() {
        let next = chars.peek().map(|&(_, next)| next);
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                chars.next();
            }
            continue;
        }
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
            continue;
        }
        match ch {
            '/' if next == Some('/') => break,
            '/' if next == Some('*') => {
                in_block_comment = true;
                chars.next();
            }
            '"' | '\'' => quote = Some(ch),
            '{' => stack.push(offset + 1),
            '}' => {
                stack.pop();
            }
            _ => {}
        }
    }
    let mut after = stack.pop()?;
    loop {
        after += line[after..].len() - line[after..].trimmed_start().len();
        let Some(comment) = line[after..].strip_prefix("/*") else {
            break;
        };
        after += 2 + comment.find("*/")? + 2;
    }
    (after < line.trimmed_end().len()).then_some(after)
}

pub(crate) fn has_unmatched_open_brace(line: &str) -> bool {
    line_brace_imbalance(line).1 > 0
}

// A middle line of a multiline block comment has no lexical marker of its own.
pub(crate) fn reverse_scan_skips_block_comment(trimmed: &str, in_block_comment: &mut bool) -> bool {
    if *in_block_comment {
        if trimmed.contains("/*") {
            *in_block_comment = false;
        }
        return true;
    }
    let line = trimmed.trimmed_end();
    if let Some(body) = line.strip_suffix("*/")
        && !body.contains("/*")
    {
        *in_block_comment = true;
        return true;
    }
    false
}

pub(crate) fn inline_brace_pair_range(line: &str) -> Option<(usize, usize)> {
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

pub(crate) fn trailing_comment_split_limit(line: &str) -> usize {
    trailing_comment_start(line)
        .map(|index| line[..index].trimmed_end().len())
        .unwrap_or(line.len())
}

pub(crate) fn line_comment_split_limit(line: &str) -> usize {
    line_comment_start(line)
        .map(|index| line[..index].trimmed_end().len())
        .unwrap_or(line.len())
}

pub(crate) fn trailing_comment_start(line: &str) -> Option<usize> {
    if !has_comment_opener(line) {
        return None;
    }
    thread_local! {
        // Layout reads the same recent lines over and over.
        static CACHE: std::cell::RefCell<LineCache<Option<usize>>> =
            std::cell::RefCell::new(LineCache::default());
    }
    if let Some(start) = CACHE.with(|cache| cache.borrow().get(line).copied()) {
        return start;
    }
    let start = match line_tokens_hold_comment(line) {
        Some(false) => None,
        _ => trailing_comment_start_in_tokens(line, true),
    };
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 4096 {
            cache.clear();
        }
        cache.insert(line.to_owned(), start);
    });
    start
}

/// Byte index of the first `*/` in `line`.
fn find_comment_close(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut from = 0;
    while let Some(offset) = find_byte(&bytes[from..], b'*') {
        let star = from + offset;
        if bytes.get(star + 1) == Some(&b'/') {
            return Some(star);
        }
        from = star + 1;
    }
    None
}

/// Whether `line` holds `//` or `/*`, in one pass.
pub(crate) fn has_comment_opener(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut from = 0;
    while let Some(offset) = find_byte(&bytes[from..], b'/') {
        let slash = from + offset;
        if matches!(bytes.get(slash + 1), Some(b'/' | b'*')) {
            return true;
        }
        from = slash + 1;
    }
    false
}

fn trailing_comment_start_in_tokens(line: &str, inspect_preprocessor: bool) -> Option<usize> {
    let mut offset = 0usize;
    let mut trailing_start = None;
    for token in tokenize(line) {
        let length = token_text(&token).len();
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
        offset += length;
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

pub(crate) fn preprocessor_directive(line: &str) -> Option<&str> {
    let rest = line.trimmed_start().strip_prefix('#')?.trimmed_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

/// Advances a scan that is inside a quoted literal by one character: tracks
/// backslash escapes and leaves the literal at its closing quote.
pub(crate) fn advance_quoted_literal(ch: char, quote: &mut Option<char>, escaped: &mut bool) {
    if *escaped {
        *escaped = false;
    } else if ch == '\\' {
        *escaped = true;
    } else if Some(ch) == *quote {
        *quote = None;
    }
}

pub(crate) fn has_hash_outside_literals(line: &str) -> bool {
    line.contains('#')
        && tokenize(line).iter().any(|token| match token {
            Token::StringLiteral(_) | Token::CharLiteral(_) | Token::Comment(..) => false,
            other => token_text(other).contains('#'),
        })
}

/// Whether `line` holds the word `word` as code, outside literals and
/// comments.
pub(crate) fn code_holds_word(line: &str, word: &str) -> bool {
    line.contains(word)
        && tokenize(line)
            .iter()
            .any(|token| matches!(token, Token::Word(text) if text == word))
}

#[cfg(test)]
mod tests {
    #[test]
    fn find_byte_finds_the_first_match() {
        let texts = [
            "",
            "/",
            "a/b//c",
            "abcdefgh/",
            "abcdefg/h/",
            "\u{80}\u{ff}//",
            "        x = y; // tail",
            "0123456789abcdef0123456789abcdef/",
            "é/ü/ß",
            "\x01\u{80}\u{81}\x7f\x2e\x2f\x30",
        ];
        for text in texts {
            for byte in [b'/', b'x', b'\x80', b'\x01', b'0', b'\xc3'] {
                assert_eq!(
                    super::find_byte(text.as_bytes(), byte),
                    text.bytes().position(|found| found == byte),
                    "{text:?} {byte}"
                );
            }
            for needle in ["", "/", "//", "x =", "ü/", "0123", "abcdef0"] {
                assert_eq!(
                    super::ContainsAnyByte::contains_from_first_byte(text, needle),
                    text.contains(needle),
                    "{text:?} {needle:?}"
                );
                assert_eq!(
                    super::ContainsAnyByte::find_from_first_byte(text, needle),
                    text.find(needle),
                    "{text:?} {needle:?}"
                );
            }
        }
    }

    fn last_unmatched_open_delimiter_by_chars(line: &str) -> Option<(char, usize)> {
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
            if quote.is_some() {
                advance_quoted_literal(ch, &mut quote, &mut escaped);
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
            if ch == '"' || (ch == '\'' && !crate::source::lex::is_digit_separator(&chars, index)) {
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

    #[test]
    fn delimiter_scan_read_as_its_line_grows_matches_a_scan_of_each_prefix() {
        let pieces = [
            "(", ")", "[", "]", "\"", "'", "/", "*", "\\", "a", "1", " ", "é", "//", "/*", "*/",
        ];
        let mut state = 0x6c07_8965_u32;
        for _ in 0..3000 {
            let mut line = String::new();
            let mut scan = DelimiterScan::default();
            for _ in 0..(state % 24) {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                line.push_str(pieces[state as usize % pieces.len()]);
                let bytes = line.as_bytes();
                scan.advance(bytes, bytes.len() - 1);
                let mut last = scan.clone();
                last.advance(bytes, bytes.len());
                assert_eq!(
                    last.last_open(),
                    last_unmatched_open_delimiter(&line),
                    "{line:?}"
                );
                assert_eq!(
                    last.last_open(),
                    last_unmatched_open_delimiter_by_chars(&line),
                    "{line:?}"
                );
            }
            state = state.wrapping_add(1);
        }
    }

    use super::*;

    #[test]
    fn classifies_whole_comment_lines() {
        for line in ["// line", " /* block", "* body", "*/"] {
            assert!(is_comment_line(line), "{line}");
        }
        for line in ["call(); // trailing", "value * other", "*p = x;", ""] {
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

/// A map from line text to what a scan found in it; lines are short and
/// hashed often, so a multiply-rotate hash serves better than SipHash.
type LineCache<V> = std::collections::HashMap<String, V, std::hash::BuildHasherDefault<LineHasher>>;

#[derive(Default)]
struct LineHasher(u64);

impl std::hash::Hasher for LineHasher {
    fn write(&mut self, bytes: &[u8]) {
        const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let word = u64::from_le_bytes(chunk.try_into().expect("eight bytes"));
            self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
        }
        for &byte in chunks.remainder() {
            self.0 = (self.0.rotate_left(5) ^ u64::from(byte)).wrapping_mul(SEED);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}
