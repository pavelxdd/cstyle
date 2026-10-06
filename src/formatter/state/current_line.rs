use crate::formatter::structure::{LineComments, TokenSpan};
use crate::formatter::text::columns::visual_width_from;
use crate::formatter::text::line_scan::DelimiterScan;
use crate::formatter::text::trim::Trimmed;
use std::cell::{Cell, RefCell};
use std::ops::Deref;

#[derive(Default)]
pub(crate) struct CurrentLine {
    text: String,
    char_len: Cell<Option<(usize, usize)>>,
    open_brace_run_len: Cell<Option<usize>>,
    blank: Cell<Option<(usize, bool)>>,
    visual_width: Cell<Option<(usize, usize)>>,
    visual_width_from: Cell<Option<(usize, usize, usize)>>,
    last_open_brace: Cell<Option<(usize, Option<usize>)>>,
    trailing_comment: Cell<Option<TrailingCommentScan>>,
    declaration_segment: RefCell<DeclarationSegmentScan>,
    marks: Cell<LineMarks>,
    parens: RefCell<ParenScan>,
    delimiters: RefCell<DelimiterScan>,
    /// Code tokens whose text is on the line so far.
    tokens: Option<TokenSpan>,
    /// Code token being pushed; text added meanwhile belongs to it.
    active_token: Option<usize>,
    /// Block comments whose text is on the line so far.
    comments: LineComments,
    /// Block comment token being pushed.
    active_comment: Option<usize>,
}

impl CurrentLine {
    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    pub(crate) fn push(&mut self, ch: char) {
        let leads = self.leads_comment();
        self.text.push(ch);
        if !ch.is_whitespace() {
            self.record_active_token(leads);
        }
    }

    pub(crate) fn push_str(&mut self, text: &str) {
        let leads = self.leads_comment();
        self.text.push_str(text);
        if !text.trimmed().is_empty() {
            self.record_active_token(leads);
        }
    }

    pub(crate) fn pop(&mut self) -> Option<char> {
        let popped = self.text.pop();
        if popped.is_some() {
            self.invalidate();
        }
        popped
    }

    pub(crate) fn truncate(&mut self, new_len: usize) {
        if new_len >= self.text.len() {
            return;
        }
        self.text.truncate(new_len);
        self.invalidate();
    }

    pub(crate) fn insert(&mut self, index: usize, ch: char) {
        let leads = self.leads_comment() && self.text[..index].trimmed().is_empty();
        self.text.insert(index, ch);
        self.invalidate();
        if !ch.is_whitespace() {
            self.record_active_token(leads);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.tokens = None;
        self.comments = LineComments::default();
        if self.text.is_empty() {
            return;
        }
        self.text.clear();
        self.invalidate();
    }

    pub(crate) fn replace(&mut self, text: String) {
        self.text = text;
        self.invalidate();
        if self.text.trimmed().is_empty() {
            self.tokens = None;
            self.comments = LineComments::default();
        } else {
            self.record_active_token(false);
        }
    }

    pub(crate) fn take(&mut self) -> String {
        self.invalidate();
        std::mem::take(&mut self.text)
    }

    /// Attributes text added from now on to the code token at `index`;
    /// `None` stops attributing.
    pub(crate) fn set_active_token(&mut self, index: Option<usize>) {
        self.active_token = index;
    }

    /// Code token being pushed, if any.
    pub(crate) fn active_token(&self) -> Option<usize> {
        self.active_token
    }

    /// Block comment token being pushed, if any.
    pub(crate) fn active_comment(&self) -> Option<usize> {
        self.active_comment
    }

    /// Attributes text added from now on to the block comment token at
    /// `index`; `None` stops attributing.
    pub(crate) fn set_active_comment(&mut self, index: Option<usize>) {
        self.active_comment = index;
    }

    /// Whether text added now would be the first text of the line and belong
    /// to a block comment.
    fn leads_comment(&self) -> bool {
        self.active_comment.is_some() && self.text.trimmed().is_empty()
    }

    fn record_active_token(&mut self, leads_comment: bool) {
        if let Some(index) = self.active_token {
            self.record_token(index);
        }
        if let Some(index) = self.active_comment {
            if leads_comment && self.comments.lead.is_none() {
                self.comments.lead = Some(index);
            }
            self.comments.last = Some(index);
        }
    }

    /// Records that the code token at `index` is part of the line.
    fn record_token(&mut self, index: usize) {
        self.tokens = Some(match self.tokens {
            Some(span) => TokenSpan {
                first: span.first.min(index),
                last: span.last.max(index),
            },
            None => TokenSpan {
                first: index,
                last: index,
            },
        });
    }

    /// Adds the code tokens of a published line taken back into this one.
    pub(crate) fn restore_tokens(&mut self, span: Option<TokenSpan>) {
        if let Some(span) = span {
            self.record_token(span.first);
            self.record_token(span.last);
        }
    }

    pub(crate) fn tokens(&self) -> Option<TokenSpan> {
        self.tokens
    }

    pub(crate) fn take_tokens(&mut self) -> Option<TokenSpan> {
        self.tokens.take()
    }

    pub(crate) fn take_comments(&mut self) -> LineComments {
        std::mem::take(&mut self.comments)
    }

    pub(crate) fn ensure_space(&mut self) {
        if !self.text.is_empty() && !self.text.ends_with(' ') {
            self.text.push(' ');
        }
    }

    pub(crate) fn trim_end_spaces(&mut self) {
        self.trim_end_matching(|ch| ch == ' ');
    }

    pub(crate) fn trim_end_horizontal_space(&mut self) {
        self.trim_end_matching(|ch| matches!(ch, ' ' | '\t'));
    }

    fn trim_end_matching(&mut self, matches: impl Fn(char) -> bool) {
        let Some(last) = self.text.chars().next_back() else {
            return;
        };
        if !matches(last) {
            return;
        }
        let before_chars = self.char_len();
        let mut popped = 0;
        while self.text.chars().next_back().is_some_and(&matches) {
            self.text.pop();
            popped += 1;
        }
        self.invalidate();
        self.char_len
            .set(Some((self.text.len(), before_chars - popped)));
    }

    pub(crate) fn char_len(&self) -> usize {
        let len = self.text.len();
        if let Some((cached_bytes, cached_chars)) = self.char_len.get() {
            if cached_bytes == len {
                return cached_chars;
            }
            if cached_bytes < len && self.text.is_char_boundary(cached_bytes) {
                let total = cached_chars + self.text[cached_bytes..].chars().count();
                debug_assert_eq!(total, self.text.chars().count());
                self.char_len.set(Some((len, total)));
                return total;
            }
        }
        let total = self.text.chars().count();
        self.char_len.set(Some((len, total)));
        total
    }

    pub(crate) fn is_blank(&self) -> bool {
        let len = self.text.len();
        if let Some((cached_bytes, cached_blank)) = self.blank.get()
            && cached_bytes == len
        {
            return cached_blank;
        }
        let blank = self.text.trimmed().is_empty();
        self.blank.set(Some((len, blank)));
        blank
    }

    pub(crate) fn visual_width(&self, tab_width: usize) -> usize {
        let len = self.text.len();
        if let Some((cached_bytes, cached_width)) = self.visual_width.get() {
            if cached_bytes == len {
                return cached_width;
            }
            if cached_bytes < len && self.text.is_char_boundary(cached_bytes) {
                let width = cached_width
                    + visual_width_from(&self.text[cached_bytes..], cached_width, tab_width);
                self.visual_width.set(Some((len, width)));
                return width;
            }
        }
        let width = visual_width_from(&self.text, 0, tab_width);
        self.visual_width.set(Some((len, width)));
        width
    }

    pub(crate) fn visual_width_from(&self, start_column: usize, tab_width: usize) -> usize {
        let len = self.text.len();
        if let Some((cached_bytes, cached_start, cached_width)) = self.visual_width_from.get()
            && cached_start == start_column
        {
            if cached_bytes == len {
                return cached_width;
            }
            if cached_bytes < len && self.text.is_char_boundary(cached_bytes) {
                let width = cached_width
                    + visual_width_from(
                        &self.text[cached_bytes..],
                        start_column + cached_width,
                        tab_width,
                    );
                self.visual_width_from.set(Some((len, start_column, width)));
                return width;
            }
        }
        let width = visual_width_from(&self.text, start_column, tab_width);
        self.visual_width_from.set(Some((len, start_column, width)));
        width
    }

    pub(crate) fn last_open_brace(&self) -> Option<usize> {
        let len = self.text.len();
        if let Some((cached_bytes, cached_index)) = self.last_open_brace.get() {
            if cached_bytes == len {
                return cached_index;
            }
            if cached_bytes < len && self.text.is_char_boundary(cached_bytes) {
                let index = self.text[cached_bytes..]
                    .rfind('{')
                    .map(|index| cached_bytes + index)
                    .or(cached_index);
                self.last_open_brace.set(Some((len, index)));
                return index;
            }
        }
        let index = self.text.rfind('{');
        self.last_open_brace.set(Some((len, index)));
        index
    }

    pub(crate) fn trailing_comment_split_limit(&self) -> usize {
        let len = self.text.len();
        let mut scan = match self.trailing_comment.get() {
            Some(scan) if scan.scanned <= len && self.text.is_char_boundary(scan.scanned) => scan,
            _ => TrailingCommentScan::default(),
        };
        if scan.comment_start.is_none() {
            scan.advance(self.text.as_bytes());
            self.trailing_comment.set(Some(scan));
        }
        match scan.comment_start {
            Some(index) => self.text[..index].trimmed_end().len(),
            None => len,
        }
    }

    /// Where the declaration the line ends in starts: past the last `,`,
    /// `;`, `{` or `}` outside parentheses and angles, or past the open
    /// parenthesis the line ends inside.
    pub(crate) fn declaration_segment_start(&self) -> usize {
        let mut scan = self.declaration_segment.borrow_mut();
        if scan.scanned > self.text.len() {
            *scan = DeclarationSegmentScan::default();
        }
        scan.advance(self.text.as_bytes());
        scan.start
    }

    fn marks(&self) -> LineMarks {
        let mut marks = self.marks.get();
        if marks.scanned > self.text.len() {
            marks = LineMarks::default();
        }
        if marks.scanned < self.text.len() {
            marks.advance(self.text.as_bytes());
            self.marks.set(marks);
        }
        marks
    }

    /// Where the statement the line ends in starts: past the last `{` or
    /// `;` outside literals.
    pub(crate) fn statement_start(&self) -> usize {
        self.marks().statement_start
    }

    /// `statement_tail` of the line.
    pub(crate) fn statement_tail(&self) -> &str {
        &self.text[self.marks().tail_start..]
    }

    /// `last_unmatched_open_delimiter` of the line, read on from where the
    /// last call stopped; the last byte is read apart, as the byte after it
    /// may change what it means.
    pub(crate) fn last_unmatched_open_delimiter(&self) -> Option<(char, usize)> {
        let bytes = self.text.as_bytes();
        let mut scan = self.delimiters.borrow_mut();
        if scan.scanned() > bytes.len() {
            *scan = DelimiterScan::default();
        }
        scan.advance(bytes, bytes.len().saturating_sub(1));
        if scan.scanned() < bytes.len() {
            let mut last = scan.clone();
            last.advance(bytes, bytes.len());
            return last.last_open();
        }
        scan.last_open()
    }

    /// The byte index of the `(` matching the last `)` of the line.
    pub(crate) fn last_close_paren_match(&self) -> Option<usize> {
        let mut scan = self.parens.borrow_mut();
        if scan.scanned > self.text.len() {
            *scan = ParenScan::default();
        }
        scan.advance(self.text.as_bytes());
        scan.last_close_match
    }

    /// Whether a `[` follows the last `]`.
    pub(crate) fn has_unclosed_bracket(&self) -> bool {
        let marks = self.marks();
        marks
            .last_open_bracket
            .is_some_and(|open| marks.last_close_bracket.is_none_or(|close| close < open))
    }

    /// The byte index of the last `?`.
    pub(crate) fn last_question(&self) -> Option<usize> {
        self.marks().last_question
    }

    /// The byte index of the last `(`, literals or not.
    pub(crate) fn last_open_paren(&self) -> Option<usize> {
        self.marks().last_open_paren
    }

    /// Whether the line holds `=`.
    pub(crate) fn holds_equals(&self) -> bool {
        self.marks().equals
    }

    /// Whether the line holds `@ {`, which opens a dictionary literal.
    pub(crate) fn holds_dictionary_opener(&self) -> bool {
        self.marks().dictionary_opener
    }

    /// Whether the line holds `asm(` or `__asm__(`.
    pub(crate) fn holds_asm_call(&self) -> bool {
        self.marks().asm_call
    }

    pub(crate) fn is_open_brace_run(&self) -> bool {
        if self.open_brace_run_len.get() == Some(self.text.len()) {
            return true;
        }
        let trimmed = self.text.trimmed_start();
        !trimmed.is_empty()
            && trimmed
                .chars()
                .all(|ch| ch == '{' || ch == ' ' || ch == '\t')
    }

    pub(crate) fn mark_open_brace_run(&self) {
        self.open_brace_run_len.set(Some(self.text.len()));
    }

    fn invalidate(&self) {
        self.char_len.set(None);
        self.open_brace_run_len.set(None);
        self.blank.set(None);
        self.visual_width.set(None);
        self.visual_width_from.set(None);
        self.last_open_brace.set(None);
        self.trailing_comment.set(None);
        self.declaration_segment.take();
        self.marks.take();
        self.parens.take();
        self.delimiters.take();
    }
}

impl Deref for CurrentLine {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

/// The open parentheses of the line read so far, and the match of the
/// last `)`.
#[derive(Default)]
struct ParenScan {
    scanned: usize,
    open: Vec<usize>,
    last_close_match: Option<usize>,
}

impl ParenScan {
    fn advance(&mut self, bytes: &[u8]) {
        for (index, &byte) in bytes.iter().enumerate().skip(self.scanned) {
            match byte {
                b'(' => self.open.push(index),
                b')' => self.last_close_match = self.open.pop(),
                _ => {}
            }
        }
        self.scanned = bytes.len();
    }
}

/// Marks of the line read once as it grows.
#[derive(Clone, Copy, Default)]
struct LineMarks {
    scanned: usize,
    quote: Option<u8>,
    escaped: bool,
    statement_start: usize,
    /// Past the last `;`, `{`, or `}`, literals or not.
    tail_start: usize,
    last_open_bracket: Option<usize>,
    last_close_bracket: Option<usize>,
    last_question: Option<usize>,
    last_open_paren: Option<usize>,
    equals: bool,
    dictionary_opener: bool,
    asm_call: bool,
}

impl LineMarks {
    fn advance(&mut self, bytes: &[u8]) {
        // A text sought may start before the bytes read last.
        let tail = &bytes[self.scanned.saturating_sub(7)..];
        self.dictionary_opener |= tail.windows(3).any(|window| window == b"@ {");
        self.asm_call |= tail.windows(4).any(|window| window == b"asm(")
            || tail.windows(8).any(|window| window == b"__asm__(");
        for (index, &byte) in bytes.iter().enumerate().skip(self.scanned) {
            match byte {
                b'[' => self.last_open_bracket = Some(index),
                b']' => self.last_close_bracket = Some(index),
                b'?' => self.last_question = Some(index),
                b'=' => self.equals = true,
                b'(' => self.last_open_paren = Some(index),
                b';' | b'{' | b'}' => self.tail_start = index + 1,
                _ => {}
            }
            if let Some(open) = self.quote {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == open {
                    self.quote = None;
                }
                continue;
            }
            match byte {
                b'"' | b'\'' => self.quote = Some(byte),
                b'{' | b';' => self.statement_start = index + 1,
                _ => {}
            }
        }
        self.scanned = bytes.len();
    }
}

#[derive(Default)]
struct DeclarationSegmentScan {
    scanned: usize,
    start: usize,
    saved_starts: Vec<usize>,
    angle_depth: u32,
}

impl DeclarationSegmentScan {
    fn advance(&mut self, bytes: &[u8]) {
        for (index, byte) in bytes.iter().enumerate().skip(self.scanned) {
            match byte {
                b'(' => {
                    self.saved_starts.push(self.start);
                    self.start = index + 1;
                }
                b')' => {
                    if let Some(previous) = self.saved_starts.pop() {
                        self.start = previous;
                    }
                }
                b'<' => self.angle_depth += 1,
                b'>' => self.angle_depth = self.angle_depth.saturating_sub(1),
                b',' | b';' | b'{' | b'}'
                    if self.saved_starts.is_empty() && self.angle_depth == 0 =>
                {
                    self.start = index + 1;
                }
                _ => {}
            }
        }
        self.scanned = bytes.len();
    }
}

#[derive(Clone, Copy, Default)]
struct TrailingCommentScan {
    scanned: usize,
    quote: Option<u8>,
    escaped: bool,
    comment_start: Option<usize>,
}

impl TrailingCommentScan {
    fn advance(&mut self, bytes: &[u8]) {
        let mut index = self.scanned;
        while index < bytes.len() {
            let ch = bytes[index];
            if let Some(quote_char) = self.quote {
                if self.escaped {
                    self.escaped = false;
                } else if ch == b'\\' {
                    self.escaped = true;
                } else if ch == quote_char {
                    self.quote = None;
                }
                index += 1;
                continue;
            }
            match ch {
                b'"' | b'\'' => self.quote = Some(ch),
                b'/' => match bytes.get(index + 1) {
                    Some(b'/') | Some(b'*') => {
                        self.comment_start = Some(index);
                        self.scanned = index;
                        return;
                    }
                    None => {
                        self.scanned = index;
                        return;
                    }
                    _ => {}
                },
                _ => {}
            }
            index += 1;
        }
        self.scanned = bytes.len();
    }
}

#[cfg(test)]
mod tests {
    use super::CurrentLine;

    #[test]
    fn blank_cache_rechecks_after_trim_and_same_length_push() {
        let mut current = CurrentLine::default();
        current.push(' ');
        assert!(current.is_blank());

        current.trim_end_spaces();
        current.push('x');

        assert!(!current.is_blank());
    }

    #[test]
    fn blank_cache_rechecks_after_same_length_replacement() {
        let mut current = CurrentLine::default();
        current.push(' ');
        assert!(current.is_blank());

        current.pop();
        current.push('x');

        assert!(!current.is_blank());
    }
}
