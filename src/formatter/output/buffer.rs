use crate::formatter::lexer::{Token, token_text, tokenize};
use crate::formatter::structure::{LineComments, TokenSpan};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{line_brace_imbalance, line_paren_imbalance};
use crate::source::lex::{is_identifier_continue, is_identifier_start};
use std::borrow::Cow;
use std::cell::{Cell, OnceCell};
use std::ops::Deref;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenBraceShape {
    Isolated,
    Label,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LineBraceMeta {
    pub(crate) code_starts_with_hash: bool,
    pub(crate) closes: usize,
    pub(crate) opens: usize,
    pub(crate) open_shape: OpenBraceShape,
    trim_start_byte: usize,
    trim_end_byte: usize,
    code_end_byte: usize,
    pub(crate) paren_closes: usize,
    pub(crate) paren_open_count: usize,
    pub(crate) paren_last_open_column: Option<usize>,
}

fn is_raw_literal(token: &Token) -> bool {
    matches!(token, Token::StringLiteral(literal) if ["u8R\"", "LR\"", "uR\"", "UR\"", "R\""]
        .into_iter()
        .any(|prefix| literal.starts_with(prefix)))
}

fn structural_line(line: &str) -> Cow<'_, str> {
    if !line.contains("R\"") && !line.contains('/') {
        return Cow::Borrowed(line);
    }
    let tokens = tokenize(line);
    if !tokens
        .iter()
        .any(|token| is_raw_literal(token) || matches!(token, Token::Comment(_, _)))
    {
        return Cow::Borrowed(line);
    }

    let mut structural = String::with_capacity(line.len());
    for token in &tokens {
        let text = token_text(token);
        if is_raw_literal(token) || matches!(token, Token::Comment(_, _)) {
            structural.extend(std::iter::repeat_n(' ', text.len()));
        } else {
            structural.push_str(&text);
        }
    }
    Cow::Owned(structural)
}

fn compute_line_brace_meta(line: &str) -> LineBraceMeta {
    let structural = structural_line(line);
    let code = structural.trim_end();
    let trimmed = code.trim_start();
    let (closes, opens) = line_brace_imbalance(code);
    let (paren_closes, paren_opens) = line_paren_imbalance(code);
    let open_shape = if trimmed == "{" {
        OpenBraceShape::Isolated
    } else if code.ends_with('{')
        && trimmed.split_once(':').is_some_and(|(label, _)| {
            let mut chars = label.chars();
            chars.next().is_some_and(is_identifier_start) && chars.all(is_identifier_continue)
        })
    {
        OpenBraceShape::Label
    } else {
        OpenBraceShape::Other
    };
    LineBraceMeta {
        code_starts_with_hash: trimmed.starts_with('#'),
        closes,
        opens,
        open_shape,
        trim_start_byte: line.len() - line.trim_start().len(),
        trim_end_byte: line.trim_end().len(),
        code_end_byte: code.len(),
        paren_closes,
        paren_open_count: paren_opens.len(),
        paren_last_open_column: paren_opens.last().copied(),
    }
}

fn compute_raw_literal_line_meta(line: &str, structural_start: usize) -> LineBraceMeta {
    let suffix = line.get(structural_start..).unwrap_or("");
    let structural = structural_line(suffix);
    let code = structural.trim_end();
    let (closes, opens) = line_brace_imbalance(code);
    let (paren_closes, paren_opens) = line_paren_imbalance(code);
    LineBraceMeta {
        code_starts_with_hash: code.trim_start().starts_with('#'),
        closes,
        opens,
        open_shape: OpenBraceShape::Other,
        trim_start_byte: line.len() - line.trim_start().len(),
        trim_end_byte: line.trim_end().len(),
        code_end_byte: line.len(),
        paren_closes,
        paren_open_count: paren_opens.len(),
        paren_last_open_column: None,
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OutputLineHints {
    has_colon: bool,
    has_else: bool,
    has_hash: bool,
    has_slash: bool,
    has_question: bool,
    starts_star: bool,
}

pub(super) fn output_line_hints(line: &str) -> OutputLineHints {
    let bytes = line.as_bytes();
    let mut hints = OutputLineHints::default();
    let mut first_non_space = None;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if first_non_space.is_none() && byte != b' ' && byte != b'\t' {
            first_non_space = Some(byte);
        }
        match byte {
            b':' => hints.has_colon = true,
            b'#' => hints.has_hash = true,
            b'/' => hints.has_slash = true,
            b'?' => hints.has_question = true,
            b'e' if bytes[index..].starts_with(b"else") => hints.has_else = true,
            _ => {}
        }
    }
    hints.starts_star = first_non_space == Some(b'*');
    hints
}

// Reads go through `Deref`; mutations stay on this type so cached line metadata cannot go stale.
#[derive(Default)]
pub(crate) struct OutputBuffer {
    lines: Vec<String>,
    meta: Vec<OnceCell<LineBraceMeta>>,
    /// Source tokens of each line, when the line came from one current line.
    tokens: Vec<Option<TokenSpan>>,
    /// Tokens of the current line just taken, for the next pushed line.
    pending_tokens: Option<TokenSpan>,
    /// Block comments of each line.
    comments: Vec<LineComments>,
    /// Comments of the current line just taken, for the next pushed line.
    pending_comments: Option<LineComments>,
    /// Block comment token being pushed; lines pushed without a current line
    /// meanwhile hold its text.
    active_comment: Option<usize>,
    may_have_label_open: bool,
    may_have_else: bool,
    may_have_hash: bool,
    may_have_comment: bool,
    may_have_question: bool,
    last_non_empty_index: Cell<Option<usize>>,
    last_non_empty_dirty: Cell<bool>,
}

impl OutputBuffer {
    fn record_hints(&mut self, line: &str, hints: OutputLineHints) {
        if hints.has_colon && line.trim_end().ends_with('{') {
            let meta = compute_line_brace_meta(line);
            self.may_have_label_open |= meta.open_shape == OpenBraceShape::Label;
        }
        self.may_have_else |= hints.has_else;
        self.may_have_hash |= hints.has_hash;
        self.may_have_comment |= hints.has_slash || hints.starts_star;
        self.may_have_question |= hints.has_question;
    }

    pub(crate) fn push(&mut self, line: String) {
        let hints = output_line_hints(&line);
        self.push_with_hints(line, hints);
    }

    pub(super) fn push_with_hints(&mut self, line: String, hints: OutputLineHints) {
        self.record_hints(&line, hints);
        let index = self.lines.len();
        let blank = line.trim().is_empty();
        if !blank {
            self.last_non_empty_index.set(Some(index));
            self.last_non_empty_dirty.set(false);
        }
        self.lines.push(line);
        self.meta.push(OnceCell::new());
        self.tokens.push(self.pending_tokens.take());
        self.push_pending_comments(blank);
    }

    pub(super) fn push_raw_literal(&mut self, line: String, structural_start: usize) {
        let suffix = line.get(structural_start..).unwrap_or("");
        self.record_hints(suffix, output_line_hints(suffix));
        let meta = compute_raw_literal_line_meta(&line, structural_start);
        let index = self.lines.len();
        let blank = line.trim().is_empty();
        if !blank {
            self.last_non_empty_index.set(Some(index));
            self.last_non_empty_dirty.set(false);
        }
        self.lines.push(line);
        self.meta.push(OnceCell::from(meta));
        self.tokens.push(self.pending_tokens.take());
        self.push_pending_comments(blank);
    }

    fn push_pending_comments(&mut self, blank: bool) {
        let active = self.active_comment.filter(|_| !blank);
        let comments = self.pending_comments.take().unwrap_or(LineComments {
            lead: active,
            last: active,
        });
        self.comments.push(comments);
    }

    pub(crate) fn pop(&mut self) -> Option<String> {
        self.meta.pop();
        self.tokens.pop();
        self.comments.pop();
        let line = self.lines.pop();
        if line.is_some() {
            self.last_non_empty_dirty.set(true);
        }
        line
    }

    pub(crate) fn last_mut(&mut self) -> Option<&mut String> {
        if let Some(slot) = self.meta.last_mut() {
            *slot = OnceCell::new();
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.last_non_empty_dirty.set(true);
        }
        self.lines.last_mut()
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut String> {
        if let Some(slot) = self.meta.get_mut(index) {
            *slot = OnceCell::new();
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.last_non_empty_dirty.set(true);
        }
        self.lines.get_mut(index)
    }

    pub(crate) fn remove(&mut self, index: usize) -> String {
        self.meta.remove(index);
        self.tokens.remove(index);
        self.comments.remove(index);
        self.last_non_empty_dirty.set(true);
        self.lines.remove(index)
    }

    pub(crate) fn set(&mut self, index: usize, line: String) {
        let hints = output_line_hints(&line);
        self.record_hints(&line, hints);
        self.meta[index] = OnceCell::new();
        self.lines[index] = line;
        self.last_non_empty_dirty.set(true);
    }

    /// Makes `tokens` the source tokens of the next pushed line.
    pub(crate) fn set_pending_tokens(&mut self, tokens: Option<TokenSpan>) {
        self.pending_tokens = tokens;
    }

    /// Makes `comments` the block comments of the next pushed line.
    pub(crate) fn set_pending_comments(&mut self, comments: LineComments) {
        self.pending_comments = Some(comments);
    }

    pub(crate) fn clear_pending_sources(&mut self) {
        self.pending_tokens = None;
        self.pending_comments = None;
    }

    /// Makes lines pushed without pending comments hold the text of the
    /// block comment token `index`.
    pub(crate) fn set_active_comment(&mut self, index: Option<usize>) {
        self.active_comment = index;
    }

    /// Source tokens of the line being finished, before it is pushed.
    pub(crate) fn pending_tokens(&self) -> Option<TokenSpan> {
        self.pending_tokens
    }

    /// Source tokens of line `index`, when known. `first` is the first code
    /// token on the line; `last` can run past the line when the line was
    /// split after it was formed.
    pub(crate) fn line_tokens(&self, index: usize) -> Option<TokenSpan> {
        self.tokens.get(index).copied().flatten()
    }

    pub(crate) fn as_slice(&self) -> &[String] {
        &self.lines
    }

    pub(crate) fn range_mut(&mut self, range: std::ops::Range<usize>) -> &mut [String] {
        for slot in &mut self.meta[range.clone()] {
            *slot = OnceCell::new();
        }
        if !range.is_empty() {
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.last_non_empty_dirty.set(true);
        }
        &mut self.lines[range]
    }

    pub(crate) fn brace_meta(&self, index: usize) -> &LineBraceMeta {
        self.meta[index].get_or_init(|| compute_line_brace_meta(&self.lines[index]))
    }

    pub(crate) fn trimmed(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][meta.trim_start_byte..meta.trim_end_byte.max(meta.trim_start_byte)]
    }

    pub(crate) fn code(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][..meta.code_end_byte]
    }

    pub(crate) fn code_trimmed(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][meta.trim_start_byte.min(meta.code_end_byte)..meta.code_end_byte]
    }

    pub(crate) fn lead_width(&self, index: usize, tab_width: usize) -> usize {
        leading_visual_width(&self.lines[index], tab_width)
    }

    /// Index of the output line that holds the source token `token`, among
    /// lines that recorded their tokens.
    pub(crate) fn line_with_token(&self, token: usize) -> Option<usize> {
        self.tokens
            .iter()
            .rposition(|span| span.is_some_and(|span| span.first <= token))
            .filter(|&index| self.tokens[index].is_some_and(|span| span.contains(token)))
    }

    pub(crate) fn current_closing_brace_open(
        &self,
        tab_width: usize,
    ) -> Option<(usize, OpenBraceShape, &str)> {
        let mut depth = 0usize;
        for index in (0..self.lines.len()).rev() {
            let meta = self.brace_meta(index);
            let trimmed = self.code_trimmed(index);
            if depth == 0
                && meta.opens > 0
                && (trimmed.starts_with("} else") || trimmed.starts_with("}else"))
            {
                return Some((self.lead_width(index, tab_width), meta.open_shape, trimmed));
            }
            depth += meta.closes;
            if meta.opens > depth {
                return Some((self.lead_width(index, tab_width), meta.open_shape, trimmed));
            }
            depth = depth.saturating_sub(meta.opens);
        }
        None
    }

    pub(crate) fn last_non_empty_index(&self) -> Option<usize> {
        if self.last_non_empty_dirty.get() {
            self.last_non_empty_index
                .set(self.lines.iter().rposition(|line| !line.trim().is_empty()));
            self.last_non_empty_dirty.set(false);
        }
        self.last_non_empty_index.get()
    }

    /// The last non-empty line, unless it continues a block comment: the
    /// tail of a comment is no code, whatever its words.
    pub(crate) fn last_line_outside_comment(&self) -> Option<&String> {
        let index = self.last_non_empty_index()?;
        (self.comment_start_index(index) == index).then(|| &self.lines[index])
    }

    /// Index of the line that opens the comment on line `index`: a block
    /// comment continuation line maps to the line holding its `/*`.
    pub(crate) fn comment_start_index(&self, index: usize) -> usize {
        let comments = self.comments[index];
        if let Some(comment) = comments.lead {
            return self.comment_opening_line(index, comment);
        }
        if comments.last.is_some() || self.tokens[index].is_some() {
            return index;
        }
        self.text_comment_start_index(index)
    }

    /// First line up to `index` holding text of the block comment token
    /// `comment`; blank lines inside the comment record no tokens.
    fn comment_opening_line(&self, index: usize, comment: usize) -> usize {
        let mut start = index;
        for line in (0..index).rev() {
            if self.comments[line].mentions(comment) {
                start = line;
            } else if !self.lines[line].trim().is_empty() {
                break;
            }
        }
        start
    }

    /// [`Self::comment_start_index`] for lines that recorded no tokens.
    fn text_comment_start_index(&self, index: usize) -> usize {
        if !self.lines[index].trim_start().starts_with('*') {
            return index;
        }
        self.lines[..index]
            .iter()
            .rposition(|line| line.contains("/*") || line.contains("*/"))
            .filter(|&start| {
                let line = &self.lines[start];
                line.rfind("/*") > line.rfind("*/")
            })
            .unwrap_or(index)
    }

    /// Leading width of the comment on line `index`, measured at the line
    /// that opens it.
    pub(crate) fn comment_indent_width(&self, index: usize, tab_width: usize) -> usize {
        leading_visual_width(&self.lines[self.comment_start_index(index)], tab_width)
    }

    pub(crate) fn may_have_label_open(&self) -> bool {
        self.may_have_label_open
    }

    pub(crate) fn may_have_else(&self) -> bool {
        self.may_have_else
    }

    pub(crate) fn may_have_hash(&self) -> bool {
        self.may_have_hash
    }

    pub(crate) fn may_have_comment(&self) -> bool {
        self.may_have_comment
    }

    pub(crate) fn may_have_question(&self) -> bool {
        self.may_have_question
    }
}

impl Deref for OutputBuffer {
    type Target = [String];

    fn deref(&self) -> &[String] {
        &self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenBraceShape, OutputBuffer};

    #[test]
    fn label_open_shape_accepts_extension_identifiers() {
        let mut output = OutputBuffer::default();
        output.push("α$value: {".to_string());

        assert_eq!(output.brace_meta(0).open_shape, OpenBraceShape::Label);
    }

    #[test]
    fn raw_literal_suffix_updates_output_hints() {
        let mut output = OutputBuffer::default();
        let line = "R\"(body)\" else".to_string();
        let structural_start = line.find("else").expect("suffix");

        output.push_raw_literal(line, structural_start);

        assert!(output.may_have_else());
    }

    #[test]
    fn lead_width_uses_the_requested_tab_width_after_metadata_is_cached() {
        let mut output = OutputBuffer::default();
        output.push("\tvalue();".to_string());
        output.code(0);

        assert_eq!(output.lead_width(0, 8), 8);
    }

    #[test]
    fn last_non_empty_index_skips_whitespace_only_tail() {
        let mut output = OutputBuffer::default();
        output.push("value();".to_string());
        output.push("    ".to_string());

        assert_eq!(output.last_non_empty_index(), Some(0));
    }
}
