use crate::formatter::structure::{LineComments, TokenSpan};
use crate::formatter::text::columns::visual_width_from;
use crate::formatter::text::line_scan::DelimiterScan;
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::operators::AssignmentChainScan;
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
    segment_outside_parens: Cell<SegmentOutsideParens>,
    assignment_chain: Cell<Option<AssignmentChainScan>>,
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
        if text.bytes().any(|byte| !byte.is_ascii_whitespace()) {
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
        let before_len = self.text.len();
        while self.text.chars().next_back().is_some_and(&matches) {
            self.text.pop();
        }
        self.cut_whitespace(before_len);
    }

    pub(crate) fn char_len(&self) -> usize {
        // Trailing blanks are a byte each; the count kept stops short of
        // them.
        let len = self.text.len();
        let stable = self.stable_len();
        let chars = match self.char_len.get() {
            Some((bytes, chars)) if bytes == stable => chars,
            Some((bytes, chars)) if bytes < stable && self.text.is_char_boundary(bytes) => {
                chars + self.text[bytes..stable].chars().count()
            }
            _ => self.text[..stable].chars().count(),
        };
        self.char_len.set(Some((stable, chars)));
        chars + (len - stable)
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
        let stable = self.stable_len();
        let width = match self.visual_width.get() {
            Some((bytes, width)) if bytes == stable => width,
            Some((bytes, width)) if bytes < stable && self.text.is_char_boundary(bytes) => {
                width + visual_width_from(&self.text[bytes..stable], width, tab_width)
            }
            _ => visual_width_from(&self.text[..stable], 0, tab_width),
        };
        self.visual_width.set(Some((stable, width)));
        width + visual_width_from(&self.text[stable..], width, tab_width)
    }

    pub(crate) fn visual_width_from(&self, start_column: usize, tab_width: usize) -> usize {
        let stable = self.stable_len();
        let width = match self.visual_width_from.get() {
            Some((bytes, start, width)) if start == start_column && bytes == stable => width,
            Some((bytes, start, width))
                if start == start_column && bytes < stable && self.text.is_char_boundary(bytes) =>
            {
                width
                    + visual_width_from(&self.text[bytes..stable], start_column + width, tab_width)
            }
            _ => visual_width_from(&self.text[..stable], start_column, tab_width),
        };
        self.visual_width_from
            .set(Some((stable, start_column, width)));
        width + visual_width_from(&self.text[stable..], start_column + width, tab_width)
    }

    pub(crate) fn last_open_brace(&self) -> Option<usize> {
        // No `{` is a trailing blank.
        let stable = self.stable_len();
        let index = match self.last_open_brace.get() {
            Some((bytes, index)) if bytes == stable => index,
            Some((bytes, index)) if bytes < stable && self.text.is_char_boundary(bytes) => self
                .text[bytes..stable]
                .rfind('{')
                .map(|found| bytes + found)
                .or(index),
            _ => self.text[..stable].rfind('{'),
        };
        self.last_open_brace.set(Some((stable, index)));
        index
    }

    pub(crate) fn trailing_comment_split_limit(&self) -> usize {
        let len = self.text.len();
        let mut scan = match self.trailing_comment.get() {
            Some(scan) if scan.scanned <= len && self.text.is_char_boundary(scan.scanned) => scan,
            _ => TrailingCommentScan::default(),
        };
        if scan.comment_start.is_none() {
            // Trailing blanks open no comment; a scan kept short of them
            // holds when they are cut.
            scan.advance(&self.text.as_bytes()[..self.stable_len()]);
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
        scan.advance(&self.text.as_bytes()[..self.stable_len()]);
        scan.start
    }

    /// `is_pointer_declaration_segment` of the text of
    /// `declaration_segment_start` on, its balanced parentheses dropped,
    /// when that text holds no `<`, `[`, or byte past ASCII.
    pub(crate) fn declaration_segment_verdict(&self) -> Option<bool> {
        self.declaration_segment_scan().verdict()
    }

    fn declaration_segment_scan(&self) -> SegmentOutsideParens {
        let start = self.declaration_segment_start();
        let mut scan = self.segment_outside_parens.get();
        if scan.start != start || scan.scanned > self.text.len() || scan.scanned < start {
            scan = SegmentOutsideParens {
                start,
                scanned: start,
                ..SegmentOutsideParens::default()
            };
        }
        let stable = self.stable_len();
        if scan.scanned < stable {
            scan.advance(&self.text.as_bytes()[..stable]);
            self.segment_outside_parens.set(scan);
        }
        scan
    }

    fn marks(&self) -> LineMarks {
        let bytes = self.text.as_bytes();
        let stable = self.stable_len();
        let mut marks = self.marks.get();
        if marks.scanned > stable {
            marks = LineMarks::default();
        }
        if marks.scanned < stable {
            marks.advance(&bytes[..stable]);
            self.marks.set(marks);
        }
        if stable < bytes.len() {
            marks.advance(bytes);
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
        scan.advance(bytes, self.stable_len().saturating_sub(1));
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
        scan.advance(&self.text.as_bytes()[..self.stable_len()]);
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

    /// Whether no `)` follows the last `@selector(`, as
    /// `has_unclosed_delimiter_after` finds.
    pub(crate) fn has_unclosed_selector_call(&self) -> bool {
        let marks = self.marks();
        marks
            .last_selector_call
            .is_some_and(|open| marks.last_close_paren.is_none_or(|close| close < open))
    }

    /// The scan for the last chained assignment of the line's code, as far
    /// as it was read.
    pub(crate) fn assignment_chain(&self) -> Option<AssignmentChainScan> {
        self.assignment_chain.get()
    }

    pub(crate) fn set_assignment_chain(&self, scan: AssignmentChainScan) {
        self.assignment_chain.set(Some(scan));
    }

    /// Whether the line holds `//` or `/*`, literals or not.
    pub(crate) fn holds_comment_opener(&self) -> bool {
        self.marks().comment_opener
    }

    /// The byte index of the last `->`.
    pub(crate) fn last_arrow(&self) -> Option<usize> {
        self.marks().last_arrow
    }

    /// `lambda_header_has_trailing_return` of the line.
    pub(crate) fn has_arrow_after_paren(&self) -> bool {
        self.marks().arrow_after_paren
    }

    /// The byte index of the last `[`.
    pub(crate) fn last_open_bracket(&self) -> Option<usize> {
        self.marks().last_open_bracket
    }

    /// Whether the line holds `{`.
    pub(crate) fn holds_open_brace(&self) -> bool {
        self.marks().open_brace
    }

    /// Whether the line holds `}`.
    pub(crate) fn holds_close_brace(&self) -> bool {
        self.marks().close_brace
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

    /// Keeps what the caches read before the whitespace the line lost
    /// from its end, which was `before_len` bytes long: the scans read no
    /// trailing blanks.
    fn cut_whitespace(&self, before_len: usize) {
        let at = self.text.len();
        self.open_brace_run_len.set(None);
        self.blank.set(
            self.blank
                .get()
                .filter(|&(bytes, _)| bytes == before_len)
                .map(|(_, blank)| (at, blank)),
        );
    }

    /// The length of the line without its trailing whitespace, past which
    /// the scans do not read for keeps.
    fn stable_len(&self) -> usize {
        self.text.trim_ascii_end().len()
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
        self.segment_outside_parens.take();
        self.assignment_chain.take();
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

/// The bytes from a start outside balanced parentheses, read once as the
/// line grows.
#[derive(Clone, Copy, Default)]
struct SegmentOutsideParens {
    start: usize,
    scanned: usize,
    depth: u32,
    angle_or_bracket: bool,
    rejecting: bool,
    /// A byte past ASCII was read.
    wide: bool,
    /// The `:`s read last in a row, and whether a row of an odd count ended.
    colons: u32,
    odd_colons: bool,
    /// The first word, up to as many bytes as the array holds, and whether
    /// it was read to its end.
    word: [u8; 16],
    word_len: usize,
    word_ended: bool,
}

impl SegmentOutsideParens {
    fn advance(&mut self, bytes: &[u8]) {
        for &byte in &bytes[self.scanned..] {
            match byte {
                b'(' => {
                    self.depth += 1;
                    continue;
                }
                b')' if self.depth > 0 => {
                    self.depth -= 1;
                    continue;
                }
                _ if self.depth > 0 => continue,
                b'<' | b'[' => self.angle_or_bracket = true,
                b'=' | b'+' | b'-' | b'/' | b'%' | b'?' | b'!' | b'~' | b'|' | b'^' | b'>'
                | b']' | b')' => self.rejecting = true,
                _ => {}
            }
            self.wide |= !byte.is_ascii();
            if byte == b':' {
                self.colons += 1;
            } else {
                self.odd_colons |= self.colons % 2 == 1;
                self.colons = 0;
            }
            if !self.word_ended {
                if byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric() {
                    if self.word_len < self.word.len() {
                        self.word[self.word_len] = byte;
                    }
                    self.word_len += 1;
                } else if self.word_len > 0 {
                    self.word_ended = true;
                }
            }
        }
        self.scanned = bytes.len();
    }

    /// `is_pointer_declaration_segment` of the text read, its balanced
    /// parentheses dropped, when it holds no `<`, `[`, or byte past ASCII.
    fn verdict(&self) -> Option<bool> {
        if self.angle_or_bracket || self.wide {
            return None;
        }
        if self.rejecting || self.odd_colons || self.colons % 2 == 1 || self.word_len == 0 {
            return Some(false);
        }
        if self.word_len > self.word.len() {
            // No keyword is that long, and a digit leads no longer word.
            return Some(!self.word[0].is_ascii_digit());
        }
        let first = std::str::from_utf8(&self.word[..self.word_len]).expect("ASCII word");
        Some(
            !first.starts_with(|ch: char| ch.is_ascii_digit())
                && !matches!(
                    first,
                    "return" | "case" | "sizeof" | "delete" | "new" | "throw" | "else"
                )
                && !crate::formatter::syntax::language::is_header(first),
        )
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
    last_close_paren: Option<usize>,
    last_selector_call: Option<usize>,
    equals: bool,
    open_brace: bool,
    close_brace: bool,
    comment_opener: bool,
    /// Whether a `)` came right before the last `-`, blanks aside.
    minus_after_paren: bool,
    last_arrow: Option<usize>,
    arrow_after_paren: bool,
    dictionary_opener: bool,
    asm_call: bool,
}

impl LineMarks {
    fn advance(&mut self, bytes: &[u8]) {
        for (index, &byte) in bytes.iter().enumerate().skip(self.scanned) {
            match byte {
                b'[' => self.last_open_bracket = Some(index),
                b']' => self.last_close_bracket = Some(index),
                b'?' => self.last_question = Some(index),
                b'=' => self.equals = true,
                b'(' => {
                    self.last_open_paren = Some(index);
                    // The texts sought that end with the `(`, which may
                    // start before the bytes read last.
                    let head = &bytes[..=index];
                    if head.ends_with(b"@selector(") {
                        self.last_selector_call = Some(index + 1 - b"@selector(".len());
                    }
                    self.asm_call |= head.ends_with(b"asm(") || head.ends_with(b"__asm__(");
                }
                b')' => self.last_close_paren = Some(index),
                b'/' | b'*' if index > 0 && bytes[index - 1] == b'/' => {
                    self.comment_opener = true;
                }
                b'-' => {
                    self.minus_after_paren = bytes[..index].trim_ascii_end().last() == Some(&b')');
                }
                b'>' if index > 0 && bytes[index - 1] == b'-' => {
                    self.last_arrow = Some(index - 1);
                    self.arrow_after_paren |= self.minus_after_paren;
                }
                b';' => self.tail_start = index + 1,
                b'{' => {
                    self.open_brace = true;
                    self.tail_start = index + 1;
                    self.dictionary_opener |= bytes[..=index].ends_with(b"@ {");
                }
                b'}' => {
                    self.close_brace = true;
                    self.tail_start = index + 1;
                }
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
    use crate::formatter::text::trim::Trimmed;

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

    /// Every answer the caches give, as a tuple to compare.
    fn answers(current: &CurrentLine) -> String {
        format!(
            "{:?}",
            (
                (
                    current.statement_start(),
                    current.statement_tail(),
                    current.last_question(),
                    current.holds_equals(),
                    current.last_open_paren(),
                    current.has_unclosed_bracket(),
                    current.holds_open_brace(),
                    current.holds_close_brace(),
                    current.holds_comment_opener(),
                    current.has_unclosed_selector_call(),
                    current.last_arrow(),
                    current.has_arrow_after_paren(),
                ),
                (
                    current.last_close_paren_match(),
                    current.declaration_segment_start(),
                    current.declaration_segment_verdict(),
                    current.last_unmatched_open_delimiter(),
                    current.trailing_comment_split_limit(),
                    current.visual_width(4),
                    current.visual_width_from(3, 4),
                    current.last_open_brace(),
                    current.char_len(),
                    current.is_blank(),
                ),
            )
        )
    }

    #[test]
    fn caches_kept_across_cut_whitespace_answer_as_fresh_ones_do() {
        let pieces = [
            "(",
            ")",
            "[",
            "]",
            "{",
            "}",
            "\"",
            "'",
            "/",
            "*",
            "\\",
            "a",
            "=",
            ",",
            ";",
            "?",
            "<",
            ">",
            " ",
            "  ",
            "\t",
            " \t ",
            "é",
            "@selector(",
            "//",
            "/*",
            "*/",
        ];
        let mut state = 0x1b87_3593_u32;
        for _ in 0..3000 {
            let mut current = CurrentLine::default();
            for _ in 0..(state % 32) {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                match state % 7 {
                    0 => current.trim_end_spaces(),
                    1 => current.trim_end_horizontal_space(),
                    _ => current.push_str(pieces[(state >> 3) as usize % pieces.len()]),
                }
                let mut fresh = CurrentLine::default();
                fresh.replace(current.as_str().to_string());
                assert_eq!(answers(&current), answers(&fresh), "{:?}", current.as_str());
                let text = current.trimmed_end();
                assert_eq!(current.last_arrow(), text.rfind("->"), "{text:?}");
                assert_eq!(
                    current.has_arrow_after_paren(),
                    crate::formatter::braces::classification::lambda_header_has_trailing_return(
                        text
                    ),
                    "{text:?}"
                );
            }
            state = state.wrapping_add(1);
        }
    }
}
