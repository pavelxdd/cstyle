use std::borrow::Cow;
use std::cell::{Cell, OnceCell};
use std::ops::Deref;

use super::columns::leading_visual_width;
use super::line_scan::{line_brace_imbalance, line_paren_imbalance};
use super::token::{Token, token_text, tokenize};
use crate::source::lex::{is_identifier_continue, is_identifier_start};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenBraceShape {
    Isolated,
    Label,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LineBraceMeta {
    pub(super) code_starts_with_hash: bool,
    pub(super) closes: usize,
    pub(super) opens: usize,
    pub(super) open_shape: OpenBraceShape,
    pub(super) trim_start_byte: usize,
    pub(super) trim_end_byte: usize,
    pub(super) code_end_byte: usize,
    pub(super) paren_closes: usize,
    pub(super) paren_open_count: usize,
    pub(super) paren_last_open_column: Option<usize>,
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
            structural.extend(std::iter::repeat(' ').take(text.len()));
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
pub(super) struct OutputBuffer {
    lines: Vec<String>,
    meta: Vec<OnceCell<LineBraceMeta>>,
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

    pub(super) fn push(&mut self, line: String) {
        let hints = output_line_hints(&line);
        self.push_with_hints(line, hints);
    }

    pub(super) fn push_with_hints(&mut self, line: String, hints: OutputLineHints) {
        self.record_hints(&line, hints);
        let index = self.lines.len();
        if !line.trim().is_empty() {
            self.last_non_empty_index.set(Some(index));
            self.last_non_empty_dirty.set(false);
        }
        self.lines.push(line);
        self.meta.push(OnceCell::new());
    }

    pub(super) fn push_raw_literal(&mut self, line: String, structural_start: usize) {
        let suffix = line.get(structural_start..).unwrap_or("");
        self.record_hints(suffix, output_line_hints(suffix));
        let meta = compute_raw_literal_line_meta(&line, structural_start);
        let index = self.lines.len();
        if !line.trim().is_empty() {
            self.last_non_empty_index.set(Some(index));
            self.last_non_empty_dirty.set(false);
        }
        self.lines.push(line);
        self.meta.push(OnceCell::from(meta));
    }

    pub(super) fn pop(&mut self) -> Option<String> {
        self.meta.pop();
        let line = self.lines.pop();
        if line.is_some() {
            self.last_non_empty_dirty.set(true);
        }
        line
    }

    pub(super) fn last_mut(&mut self) -> Option<&mut String> {
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

    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut String> {
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

    pub(super) fn remove(&mut self, index: usize) -> String {
        self.meta.remove(index);
        self.last_non_empty_dirty.set(true);
        self.lines.remove(index)
    }

    pub(super) fn set(&mut self, index: usize, line: String) {
        let hints = output_line_hints(&line);
        self.record_hints(&line, hints);
        self.meta[index] = OnceCell::new();
        self.lines[index] = line;
        self.last_non_empty_dirty.set(true);
    }

    pub(super) fn as_slice(&self) -> &[String] {
        &self.lines
    }

    pub(super) fn range_mut(&mut self, range: std::ops::Range<usize>) -> &mut [String] {
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

    pub(super) fn brace_meta(&self, index: usize) -> &LineBraceMeta {
        self.meta[index].get_or_init(|| compute_line_brace_meta(&self.lines[index]))
    }

    pub(super) fn trimmed(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][meta.trim_start_byte..meta.trim_end_byte.max(meta.trim_start_byte)]
    }

    pub(super) fn code(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][..meta.code_end_byte]
    }

    pub(super) fn code_trimmed(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][meta.trim_start_byte.min(meta.code_end_byte)..meta.code_end_byte]
    }

    pub(super) fn lead_width(&self, index: usize, tab_width: usize) -> usize {
        leading_visual_width(&self.lines[index], tab_width)
    }

    pub(super) fn current_closing_brace_open(
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

    pub(super) fn last_non_empty_index(&self) -> Option<usize> {
        if self.last_non_empty_dirty.get() {
            self.last_non_empty_index
                .set(self.lines.iter().rposition(|line| !line.trim().is_empty()));
            self.last_non_empty_dirty.set(false);
        }
        self.last_non_empty_index.get()
    }

    pub(super) fn last_non_empty_line(&self) -> Option<&String> {
        self.last_non_empty_index().map(|index| &self.lines[index])
    }

    pub(super) fn may_have_label_open(&self) -> bool {
        self.may_have_label_open
    }

    pub(super) fn may_have_else(&self) -> bool {
        self.may_have_else
    }

    pub(super) fn may_have_hash(&self) -> bool {
        self.may_have_hash
    }

    pub(super) fn may_have_comment(&self) -> bool {
        self.may_have_comment
    }

    pub(super) fn may_have_question(&self) -> bool {
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
