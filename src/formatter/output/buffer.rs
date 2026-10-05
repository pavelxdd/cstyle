use crate::formatter::lexer::{Token, token_text, tokenize};
use crate::formatter::structure::{LineComments, TokenSpan};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{
    line_brace_imbalance, line_paren_imbalance, preprocessor_directive,
    trailing_comment_split_limit,
};
use crate::formatter::text::trim::Trimmed;
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
    /// End of the code before a trailing comment, as
    /// `trailing_comment_split_limit` finds it.
    comment_split_limit: usize,
    /// Whether the line's text holds `new `.
    mentions_new: bool,
    /// Whether the trimmed line is `else` or ends with `} else`.
    else_line: bool,
    /// The same for the line's code without comments.
    code_else_line: bool,
}

fn is_else_line(trimmed: &str) -> bool {
    trimmed == "else" || trimmed.ends_with("} else")
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
    let code = structural.trimmed_end();
    let trimmed = code.trimmed_start();
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
        trim_start_byte: line.len() - line.trimmed_start().len(),
        trim_end_byte: line.trimmed_end().len(),
        code_end_byte: code.len(),
        paren_closes,
        paren_open_count: paren_opens.len(),
        paren_last_open_column: paren_opens.last().copied(),
        comment_split_limit: trailing_comment_split_limit(line),
        mentions_new: line.contains("new "),
        else_line: is_else_line(line.trimmed()),
        code_else_line: is_else_line(
            &line[(line.len() - line.trimmed_start().len()).min(code.len())..code.len()],
        ),
    }
}

fn compute_raw_literal_line_meta(line: &str, structural_start: usize) -> LineBraceMeta {
    let suffix = line.get(structural_start..).unwrap_or("");
    let structural = structural_line(suffix);
    let code = structural.trimmed_end();
    let (closes, opens) = line_brace_imbalance(code);
    let (paren_closes, paren_opens) = line_paren_imbalance(code);
    LineBraceMeta {
        code_starts_with_hash: code.trimmed_start().starts_with('#'),
        closes,
        opens,
        open_shape: OpenBraceShape::Other,
        trim_start_byte: line.len() - line.trimmed_start().len(),
        trim_end_byte: line.trimmed_end().len(),
        code_end_byte: line.len(),
        paren_closes,
        paren_open_count: paren_opens.len(),
        paren_last_open_column: None,
        comment_split_limit: trailing_comment_split_limit(line),
        mentions_new: line.contains("new "),
        else_line: is_else_line(line.trimmed()),
        code_else_line: is_else_line(line.trimmed_start()),
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OutputLineHints {
    has_colon: bool,
    has_else: bool,
    has_hash: bool,
    has_slash: bool,
    has_question: bool,
    has_at: bool,
    has_new: bool,
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
            b'@' => hints.has_at = true,
            b'n' if bytes[index..].starts_with(b"new ") => hints.has_new = true,
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
    /// Unmatched `)`/`]` and the columns of unmatched `(`/`[` of each
    /// line, read once a layout rule asks.
    parens: Vec<OnceCell<(usize, Vec<usize>)>>,
    /// Source tokens of each line, when the line came from one current line.
    tokens: Vec<Option<TokenSpan>>,
    /// Tokens of the current line just taken, for the next pushed line.
    pending_tokens: Option<TokenSpan>,
    /// Block comments of each line.
    comments: Vec<LineComments>,
    /// Lines kept as the source wrote them: disabled regions, raw lines,
    /// and multi-line literal rows. Their whitespace is content.
    verbatim: Vec<bool>,
    /// Continued lines of a directive that the formatter indented itself.
    indented_directive_continuation: Vec<bool>,
    /// Comments of the current line just taken, for the next pushed line.
    pending_comments: Option<LineComments>,
    /// Block comment token being pushed; lines pushed without a current line
    /// meanwhile hold its text.
    active_comment: Option<usize>,
    /// First line of the current top-level construct: the line closing the
    /// previous function body.
    scope_start: usize,
    /// Line that starts the next construct once a later line is pushed, so
    /// the closing line's own layout still sees its function.
    pending_scope_start: Option<usize>,
    may_have_label_open: bool,
    may_have_else: bool,
    may_have_hash: bool,
    may_have_comment: bool,
    may_have_question: bool,
    may_have_at: bool,
    may_have_new: bool,
    /// The last answer of [`Self::recent_scoped_line_mentions_new`]: the
    /// line count, version and line count looked at, and the answer.
    mentions_new_cache: Cell<Option<(usize, u64, usize, bool)>>,
    /// The last answer of [`Self::designator_since_closed_row`]: the scope
    /// start, version and line count it read, and the answer.
    designator_cache: Cell<Option<(usize, u64, usize, bool)>>,
    last_non_empty_index: Cell<Option<usize>>,
    last_non_empty_dirty: Cell<bool>,
    /// Counts changes to lines already pushed and to the scope.
    version: u64,
    /// The last look back for the open brace a closing brace would close:
    /// the line count and version it read, and the line it found.
    closing_brace_open_cache: Cell<Option<(usize, u64, Option<usize>)>>,
    /// The last line outside comments, with the line count and version it
    /// was found for.
    last_outside_comment_cache: Cell<Option<(usize, u64, Option<usize>)>>,
    /// Largest first token of a line pushed so far.
    largest_first_token: Option<usize>,
    /// Whether a line ever recorded a first token before that of an earlier
    /// line; until then lines can be searched by token.
    first_tokens_unordered: bool,
}

impl OutputBuffer {
    fn record_hints(&mut self, line: &str, hints: OutputLineHints) {
        if hints.has_colon && line.trimmed_end().ends_with('{') {
            let meta = compute_line_brace_meta(line);
            self.may_have_label_open |= meta.open_shape == OpenBraceShape::Label;
        }
        self.may_have_else |= hints.has_else;
        self.may_have_hash |= hints.has_hash;
        self.may_have_comment |= hints.has_slash || hints.starts_star;
        self.may_have_question |= hints.has_question;
        self.may_have_at |= hints.has_at;
        self.may_have_new |= hints.has_new;
    }

    pub(crate) fn push(&mut self, line: String) {
        let hints = output_line_hints(&line);
        self.push_with_hints(line, hints);
    }

    pub(super) fn push_with_hints(&mut self, line: String, hints: OutputLineHints) {
        self.record_hints(&line, hints);
        let index = self.lines.len();
        let blank = line.trimmed().is_empty();
        if !blank {
            self.last_non_empty_index.set(Some(index));
            self.last_non_empty_dirty.set(false);
        }
        self.lines.push(line);
        self.meta.push(OnceCell::new());
        self.parens.push(OnceCell::new());
        self.push_pending_sources(blank);
    }

    pub(super) fn push_raw_literal(&mut self, line: String, structural_start: usize) {
        let suffix = line.get(structural_start..).unwrap_or("");
        self.record_hints(suffix, output_line_hints(suffix));
        self.may_have_at |= line.contains('@');
        self.may_have_new |= line.contains("new ");
        let meta = compute_raw_literal_line_meta(&line, structural_start);
        let index = self.lines.len();
        let blank = line.trimmed().is_empty();
        if !blank {
            self.last_non_empty_index.set(Some(index));
            self.last_non_empty_dirty.set(false);
        }
        self.lines.push(line);
        self.meta.push(OnceCell::from(meta));
        self.parens.push(OnceCell::new());
        self.push_pending_sources(blank);
        self.mark_last_verbatim();
    }

    /// Keeps the last pushed line as written.
    pub(crate) fn mark_last_verbatim(&mut self) {
        if let Some(last) = self.verbatim.last_mut() {
            *last = true;
        }
    }

    pub(crate) fn is_verbatim(&self, index: usize) -> bool {
        self.verbatim.get(index).copied().unwrap_or(false)
    }

    pub(crate) fn mark_last_indented_directive_continuation(&mut self) {
        if let Some(last) = self.indented_directive_continuation.last_mut() {
            *last = true;
        }
    }

    pub(crate) fn is_indented_directive_continuation(&self, index: usize) -> bool {
        self.indented_directive_continuation
            .get(index)
            .copied()
            .unwrap_or(false)
    }

    /// Gives the pending sources to the line just pushed; a blank line, such
    /// as one inserted before the line they belong to, holds none.
    fn push_pending_sources(&mut self, blank: bool) {
        if let Some(start) = self.pending_scope_start
            && self.lines.len() > start + 1
        {
            self.scope_start = start;
            self.pending_scope_start = None;
        }
        self.verbatim.push(false);
        self.indented_directive_continuation.push(false);
        if blank {
            self.tokens.push(None);
            self.comments.push(LineComments::default());
            return;
        }
        let tokens = self.pending_tokens.take();
        if let Some(span) = tokens {
            self.first_tokens_unordered |= self
                .largest_first_token
                .is_some_and(|largest| span.first < largest);
            self.largest_first_token = Some(
                self.largest_first_token
                    .map_or(span.first, |largest| largest.max(span.first)),
            );
        }
        self.tokens.push(tokens);
        let comments = self.pending_comments.take().unwrap_or(LineComments {
            lead: self.active_comment,
            last: self.active_comment,
        });
        self.comments.push(comments);
    }

    /// Removes the last line with the code tokens it held.
    pub(crate) fn pop_with_tokens(&mut self) -> Option<(String, Option<TokenSpan>)> {
        let tokens = self.tokens.last().copied().flatten();
        self.pop().map(|line| (line, tokens))
    }

    pub(crate) fn pop(&mut self) -> Option<String> {
        self.meta.pop();
        self.parens.pop();
        self.tokens.pop();
        self.comments.pop();
        self.verbatim.pop();
        self.indented_directive_continuation.pop();
        let line = self.lines.pop();
        if line.is_some() {
            self.last_non_empty_dirty.set(true);
            self.version += 1;
        }
        line
    }

    pub(crate) fn last_mut(&mut self) -> Option<&mut String> {
        if let Some(slot) = self.meta.last_mut() {
            *slot = OnceCell::new();
            if let Some(parens) = self.parens.last_mut() {
                *parens = OnceCell::new();
            }
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.may_have_at = true;
            self.may_have_new = true;
            self.last_non_empty_dirty.set(true);
            self.version += 1;
        }
        self.lines.last_mut()
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut String> {
        if let Some(slot) = self.meta.get_mut(index) {
            *slot = OnceCell::new();
            self.parens[index] = OnceCell::new();
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.may_have_at = true;
            self.may_have_new = true;
            self.last_non_empty_dirty.set(true);
            self.version += 1;
        }
        self.lines.get_mut(index)
    }

    pub(crate) fn remove(&mut self, index: usize) -> String {
        self.meta.remove(index);
        self.parens.remove(index);
        self.tokens.remove(index);
        self.comments.remove(index);
        self.verbatim.remove(index);
        self.indented_directive_continuation.remove(index);
        self.last_non_empty_dirty.set(true);
        self.version += 1;
        self.lines.remove(index)
    }

    /// Appends the text of line `from` to line `into`, after `separator`,
    /// and removes line `from`; `into` then holds the tokens of both.
    pub(crate) fn join_into(&mut self, into: usize, from: usize, separator: &str) {
        let text = self.lines[from].trimmed().to_string();
        let span = match (self.tokens[into], self.tokens[from]) {
            (Some(a), Some(b)) => Some(TokenSpan {
                first: a.first.min(b.first),
                last: a.last.max(b.last),
            }),
            (a, b) => a.or(b),
        };
        self.first_tokens_unordered |=
            span.map(|span| span.first) != self.tokens[into].map(|span| span.first);
        self.remove(from);
        let mut line = self.lines[into].trimmed_end().to_string();
        line.push_str(separator);
        line.push_str(&text);
        self.set(into, line);
        self.tokens[into] = span;
    }

    pub(crate) fn set(&mut self, index: usize, line: String) {
        let hints = output_line_hints(&line);
        self.record_hints(&line, hints);
        self.meta[index] = OnceCell::new();
        self.parens[index] = OnceCell::new();
        self.lines[index] = line;
        self.last_non_empty_dirty.set(true);
        self.version += 1;
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

    /// Changes with every edit of a line already pushed; with the line
    /// count, it identifies the buffer's content.
    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn as_slice(&self) -> &[String] {
        &self.lines
    }

    pub(crate) fn range_mut(&mut self, range: std::ops::Range<usize>) -> &mut [String] {
        for slot in &mut self.meta[range.clone()] {
            *slot = OnceCell::new();
        }
        for slot in &mut self.parens[range.clone()] {
            *slot = OnceCell::new();
        }
        if !range.is_empty() {
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.may_have_at = true;
            self.may_have_new = true;
            self.last_non_empty_dirty.set(true);
            self.version += 1;
        }
        &mut self.lines[range]
    }

    pub(crate) fn brace_meta(&self, index: usize) -> &LineBraceMeta {
        self.meta[index].get_or_init(|| {
            let line = &self.lines[index];
            // A row of a block comment holds no code.
            if self.tokens[index].is_none() && self.comments[index].lead.is_some() {
                LineBraceMeta {
                    code_starts_with_hash: false,
                    closes: 0,
                    opens: 0,
                    open_shape: OpenBraceShape::Other,
                    trim_start_byte: line.len() - line.trimmed_start().len(),
                    trim_end_byte: line.trimmed_end().len(),
                    code_end_byte: 0,
                    paren_closes: 0,
                    paren_open_count: 0,
                    paren_last_open_column: None,
                    comment_split_limit: trailing_comment_split_limit(line),
                    mentions_new: line.contains("new "),
                    else_line: is_else_line(line.trimmed()),
                    code_else_line: false,
                }
            } else {
                compute_line_brace_meta(line)
            }
        })
    }

    /// Unmatched `)`/`]` of line `index` and the columns of its unmatched
    /// `(`/`[`, outside literals and comments.
    pub(crate) fn paren_imbalance(&self, index: usize) -> (usize, &[usize]) {
        let (closes, opens) = self.parens[index]
            .get_or_init(|| line_paren_imbalance(self.lines[index].trimmed_end()));
        (*closes, opens)
    }

    /// The recent line whose code `text` is, from at most its indent to its
    /// code's end with or without trailing blanks, and the offset of `text`
    /// in it.
    pub(crate) fn code_line_of(&self, text: &str) -> Option<(usize, usize)> {
        let address = text.as_ptr() as usize;
        let recent = self.lines.len().saturating_sub(16);
        let index = (recent..self.lines.len()).rev().find(|&index| {
            let start = self.lines[index].as_ptr() as usize;
            (start..=start + self.lines[index].len()).contains(&address)
        })?;
        let line = &self.lines[index];
        let offset = address - line.as_ptr() as usize;
        let code = self.code_before_comment(index);
        let end = offset + text.len();
        (line.as_bytes()[..offset]
            .iter()
            .all(u8::is_ascii_whitespace)
            && (end == code.len() || end == code.trimmed_end().len()))
        .then_some((index, offset))
    }

    /// The last unmatched `(`/`[` of line `index` with code after it, as
    /// `unmatched_open_paren_column` finds it in the line's code.
    pub(crate) fn unmatched_open_paren_column(&self, index: usize) -> Option<usize> {
        let code = self.code_before_comment(index);
        self.paren_imbalance(index)
            .1
            .iter()
            .rev()
            .copied()
            .filter(|&column| column < code.len())
            .find(|&column| code[column + 1..].chars().any(|ch| !ch.is_whitespace()))
    }

    pub(crate) fn trimmed(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][meta.trim_start_byte..meta.trim_end_byte.max(meta.trim_start_byte)]
    }

    pub(crate) fn code(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][..meta.code_end_byte]
    }

    /// Line `index` up to its trailing comment, as
    /// `trailing_comment_split_limit` cuts it.
    pub(crate) fn code_before_comment(&self, index: usize) -> &str {
        &self.lines[index][..self.brace_meta(index).comment_split_limit]
    }

    /// `line` up to its trailing comment, as `trailing_comment_split_limit`
    /// cuts it; one of the last lines is cut from its cached metadata.
    pub(crate) fn code_of<'a>(&'a self, line: &'a str) -> &'a str {
        let recent = self.lines.len().saturating_sub(8);
        // The same bytes in memory are that line, borrowed unchanged.
        if let Some(index) = (recent..self.lines.len()).rev().find(|&index| {
            let held = &self.lines[index];
            held.as_ptr() == line.as_ptr() && held.len() == line.len()
        }) {
            return self.code_before_comment(index);
        }
        &line[..trailing_comment_split_limit(line)]
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
        let index = if self.first_tokens_unordered {
            self.tokens
                .iter()
                .rposition(|span| span.is_some_and(|span| span.first <= token))
        } else {
            self.last_line_starting_by(token)
        };
        index.filter(|&index| self.tokens[index].is_some_and(|span| span.contains(token)))
    }

    /// The last line whose first token is at most `token`, by binary search
    /// over lines recorded in token order.
    fn last_line_starting_by(&self, token: usize) -> Option<usize> {
        // The first token of the last line at or before `index` that
        // recorded its tokens, with that line.
        let first_at = |index: usize| {
            self.tokens[..=index]
                .iter()
                .rposition(Option::is_some)
                .map(|line| (self.tokens[line].map_or(0, |span| span.first), line))
        };
        let (mut low, mut high) = (0, self.tokens.len());
        while low < high {
            let middle = low + (high - low) / 2;
            if first_at(middle).is_none_or(|(first, _)| first <= token) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low.checked_sub(1).and_then(first_at).map(|(_, line)| line)
    }

    pub(crate) fn current_closing_brace_open(
        &self,
        tab_width: usize,
    ) -> Option<(usize, OpenBraceShape, &str)> {
        // Lines pushed since the last look back are read first; with no
        // brace left open among them, the walk ends where it did then.
        let len = self.lines.len();
        let cached = self
            .closing_brace_open_cache
            .get()
            .filter(|&(cached_len, version, _)| version == self.version && cached_len <= len);
        let stop = cached.map_or(0, |(cached_len, _, _)| cached_len);
        let mut depth = 0usize;
        let mut found = None;
        let mut index = len;
        while index > stop {
            index -= 1;
            if let Some(open) = self.closing_brace_open_step(index, &mut depth) {
                found = Some(Some(open));
                break;
            }
        }
        let open = match (found, cached) {
            (Some(open), _) => open,
            (None, Some((_, _, open))) if depth == 0 => open,
            _ => {
                let mut open = None;
                while index > 0 {
                    index -= 1;
                    if let Some(at) = self.closing_brace_open_step(index, &mut depth) {
                        open = Some(at);
                        break;
                    }
                }
                open
            }
        };
        self.closing_brace_open_cache
            .set(Some((len, self.version, open)));
        let index = open?;
        let meta = self.brace_meta(index);
        Some((
            self.lead_width(index, tab_width),
            meta.open_shape,
            self.code_trimmed(index),
        ))
    }

    /// One step of the walk back for the open brace the next closing brace
    /// closes: line `index`, if it holds that brace.
    fn closing_brace_open_step(&self, index: usize, depth: &mut usize) -> Option<usize> {
        // The body of a block comment holds no braces.
        if self.comment_start_index(index) != index {
            return None;
        }
        let meta = self.brace_meta(index);
        let trimmed = self.code_trimmed(index);
        if *depth == 0
            && meta.opens > 0
            && (trimmed.starts_with("} else") || trimmed.starts_with("}else"))
        {
            return Some(index);
        }
        *depth += meta.closes;
        if meta.opens > *depth {
            return Some(index);
        }
        *depth = depth.saturating_sub(meta.opens);
        None
    }

    /// Lines from the start of the current top-level construct on: layout
    /// looking back never needs the inside of an earlier function.
    pub(crate) fn scoped(&self) -> &[String] {
        &self.lines[self.scope_start.min(self.lines.len())..]
    }

    /// Indices of the lines [`Self::scoped`] holds.
    pub(crate) fn scoped_range(&self) -> std::ops::Range<usize> {
        self.scope_start.min(self.lines.len())..self.lines.len()
    }

    /// Whether one of the last `count` lines in scope is `else` or ends
    /// with `} else`.
    pub(crate) fn recent_scoped_else_line(&self, count: usize) -> bool {
        self.scoped_range()
            .rev()
            .take(count)
            .any(|index| self.brace_meta(index).else_line)
    }

    /// Whether one of the last `count` lines has the code `else` or code
    /// ending with `} else`.
    pub(crate) fn recent_code_else_line(&self, count: usize) -> bool {
        (0..self.lines.len())
            .rev()
            .take(count)
            .any(|index| self.brace_meta(index).code_else_line)
    }

    /// Whether one of the last `count` lines in scope holds `new `.
    pub(crate) fn recent_scoped_line_mentions_new(&self, count: usize) -> bool {
        if !self.may_have_new {
            return false;
        }
        if let Some((len, version, cached_count, mentions)) = self.mentions_new_cache.get()
            && (len, version, cached_count) == (self.lines.len(), self.version, count)
        {
            return mentions;
        }
        let mentions = self
            .scoped_range()
            .rev()
            .take(count)
            .any(|index| self.brace_meta(index).mentions_new);
        self.mentions_new_cache
            .set(Some((self.lines.len(), self.version, count, mentions)));
        mentions
    }

    /// Whether a line in scope led by `[` follows the last one led by `},`.
    pub(crate) fn designator_since_closed_row(&self) -> bool {
        let range = self.scoped_range();
        let key = (range.start, self.version, range.end);
        if let Some((start, version, len, designator)) = self.designator_cache.get()
            && (start, version, len) == key
        {
            return designator;
        }
        let designator = self.lines[range]
            .iter()
            .rev()
            .map(|line| line.trimmed_start())
            .take_while(|trimmed| !trimmed.starts_with("},"))
            .any(|trimmed| trimmed.starts_with('['));
        self.designator_cache
            .set(Some((key.0, key.1, key.2, designator)));
        designator
    }

    pub(crate) fn clear_scope(&mut self) {
        self.scope_start = 0;
        self.pending_scope_start = None;
        // What layout reads back depends on the scope.
        self.version += 1;
    }

    /// Starts a new top-level construct at line `index` once a line after
    /// it is pushed.
    pub(crate) fn set_scope_start(&mut self, index: usize) {
        self.pending_scope_start = Some(index);
    }

    pub(crate) fn last_non_empty_index(&self) -> Option<usize> {
        if self.last_non_empty_dirty.get() {
            self.last_non_empty_index.set(
                self.lines
                    .iter()
                    .rposition(|line| !line.trimmed().is_empty()),
            );
            self.last_non_empty_dirty.set(false);
        }
        self.last_non_empty_index.get()
    }

    /// The last non-empty line, unless it continues a block comment: the
    /// tail of a comment is no code, whatever its words.
    pub(crate) fn last_line_outside_comment(&self) -> Option<&String> {
        let key = (self.lines.len(), self.version);
        let index = match self.last_outside_comment_cache.get() {
            Some((len, version, index)) if (len, version) == key => index,
            _ => {
                let index = self.find_last_line_outside_comment();
                self.last_outside_comment_cache
                    .set(Some((key.0, key.1, index)));
                index
            }
        };
        index.map(|index| &self.lines[index])
    }

    fn find_last_line_outside_comment(&self) -> Option<usize> {
        let index = self.last_non_empty_index()?;
        // A backslash-continued macro body is no code of the lines after it.
        if self
            .directive_of_continuation(index)
            .is_some_and(|directive| {
                preprocessor_directive(self.trimmed(directive)) == Some("define")
            })
        {
            return None;
        }
        (self.comment_start_index(index) == index).then_some(index)
    }

    /// The last non-empty line in the current scope that is not only a
    /// comment: comments take no part in the layout of the code around them.
    pub(crate) fn last_code_line_in_scope(&self) -> Option<&String> {
        (self.scope_start.min(self.lines.len())..self.lines.len())
            .rev()
            .find(|&index| {
                let text = self.trimmed(index);
                !text.is_empty()
                    && (self.line_tokens(index).is_some()
                        || self.comment_start_index(index) == index
                            && !text.starts_with("/*")
                            && !text.starts_with("//"))
            })
            .map(|index| &self.lines[index])
    }

    /// The directive line that line `index` continues through
    /// backslash-newlines.
    pub(crate) fn directive_of_continuation(&self, index: usize) -> Option<usize> {
        let mut line = index;
        // A comment row of the body continues the directive too.
        while line > 0 && self.trimmed(line - 1).ends_with('\\') {
            line -= 1;
            if self.trimmed(line).starts_with('#') {
                return Some(line);
            }
        }
        None
    }

    /// Whether line `index` belongs to a directive, on its first line or on
    /// a backslash-continued one.
    pub(crate) fn is_directive_line(&self, index: usize) -> bool {
        self.trimmed(index).starts_with('#') || self.directive_of_continuation(index).is_some()
    }

    /// The first comment token recorded on line `index`.
    pub(crate) fn comment_token(&self, index: usize) -> Option<usize> {
        let comments = self.comments.get(index)?;
        comments.lead.or(comments.last)
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
            } else if !self.lines[line].trimmed().is_empty() {
                break;
            }
        }
        start
    }

    /// [`Self::comment_start_index`] for lines that recorded no tokens.
    fn text_comment_start_index(&self, index: usize) -> usize {
        if !self.lines[index].trimmed_start().starts_with('*') {
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

    /// Whether a line may hold `@`; false while no line ever did.
    pub(crate) fn may_have_at(&self) -> bool {
        self.may_have_at
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
