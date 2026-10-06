use crate::formatter::engine::FormatEngine;
use crate::formatter::state::PreviousToken;
use crate::formatter::state::frame::StringContinuationFrame;
use crate::formatter::syntax::language::is_type_like_pointer_word;
use crate::formatter::text::line_scan::{ContainsAnyByte, trailing_comment_split_limit};
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::operators::starts_with_chain_operator;
use crate::source::lex::{is_identifier_continue, trailing_word};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct LiteralLineState {
    pub(crate) is_multiline_literal: bool,
    pub(crate) multiline_literal_end: Option<usize>,
    pub(crate) unterminated_raw_literal: bool,
    pub(crate) preserve_raw_literal_line_end: bool,
    pub(crate) unterminated_literal_line: bool,
}

fn raw_literal_is_unterminated(literal: &str) -> bool {
    let Some(prefix) = ["u8R\"", "LR\"", "uR\"", "UR\"", "R\""]
        .into_iter()
        .find(|prefix| literal.starts_with(prefix))
    else {
        return false;
    };
    let after_prefix = &literal[prefix.len()..];
    let Some(open) = after_prefix.find('(') else {
        return false;
    };
    let delimiter = &after_prefix[..open];
    !literal.ends_with(&format!("){delimiter}\""))
}

pub(crate) fn first_string_literal_start(line: &str) -> Option<usize> {
    // Every literal, prefixed or not, has a `"`.
    if !line.contains('"') {
        return None;
    }
    let code = &line[..trailing_comment_split_limit(line)];
    let prefixes = [
        "u8R\"", "u8\"", "uR\"", "UR\"", "LR\"", "R\"", "u\"", "U\"", "L\"",
    ];
    let mut in_char = false;
    let mut escaped = false;
    for (index, ch) in code.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_char {
            escaped = true;
            continue;
        }
        if ch == '\'' {
            in_char = !in_char;
            continue;
        }
        if in_char {
            continue;
        }
        if ch == '"'
            || matches!(ch, 'u' | 'U' | 'L' | 'R')
                && prefixes
                    .iter()
                    .any(|prefix| code[index..].starts_with(prefix))
        {
            return Some(index);
        }
    }
    None
}

pub(crate) fn starts_string_literal_token(line: &str) -> bool {
    // A literal starts with its quote or a prefix letter.
    matches!(
        line.as_bytes().first(),
        Some(b'"' | b'u' | b'U' | b'L' | b'R')
    ) && first_string_literal_start(line) == Some(0)
}

pub(crate) fn string_literal_token_end(line: &str, start: usize) -> Option<usize> {
    let quote = line[start..].find('"')? + start;
    if line[start..quote].ends_with('R') {
        let delimiter_start = quote + 1;
        let delimiter_len = line[delimiter_start..].find('(')?;
        let delimiter = &line[delimiter_start..delimiter_start + delimiter_len];
        let body_start = delimiter_start + delimiter_len + 1;
        let close = format!("){delimiter}\"");
        return line[body_start..]
            .find(&close)
            .map(|offset| body_start + offset + close.len());
    }
    let mut escaped = false;
    for (offset, ch) in line[quote + 1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(quote + 1 + offset + ch.len_utf8());
        }
    }
    None
}

pub(crate) fn string_literal_has_opening_context(line: &str, start: usize) -> bool {
    !matches!(
        line[..start].trimmed_end().chars().next_back(),
        Some(ch) if is_identifier_continue(ch) || matches!(ch, ')' | ']')
    )
}

pub(crate) fn single_string_literal_comma_line(line: &str) -> bool {
    let Some(start) = first_string_literal_start(line) else {
        return false;
    };
    if !line[..start].trimmed().is_empty() {
        return false;
    }
    let Some(end) = string_literal_token_end(line, start) else {
        return false;
    };
    line[end..].trimmed() == ","
}

pub(crate) fn last_string_literal_start(line: &str) -> Option<usize> {
    let code = &line[..trailing_comment_split_limit(line)];
    let mut search_start = 0;
    let mut last = None;
    while search_start < code.len() {
        let Some(relative_start) = first_string_literal_start(&code[search_start..]) else {
            break;
        };
        let start = search_start + relative_start;
        last = Some(start);
        let Some(end) = string_literal_token_end(code, start) else {
            break;
        };
        if end <= search_start {
            break;
        }
        search_start = end;
    }
    last
}

impl FormatEngine<'_> {
    pub(crate) fn try_finish_multiline_literal_line(&mut self) -> bool {
        if !self.layout.literal_line.is_multiline_literal {
            return false;
        }
        let structural_start = self
            .layout
            .literal_line
            .multiline_literal_end
            .take()
            .unwrap_or(self.current.len());
        let preserve_line_end =
            self.current.contains('\x0c') || self.layout.literal_line.unterminated_raw_literal;
        let line = self.take_current();
        let line = if preserve_line_end {
            line
        } else {
            line.trimmed_end().to_string()
        };
        self.adjust_and_publish_raw_literal_line(line, structural_start);
        self.reset_after_finished_line();
        true
    }

    pub(crate) fn push_literal(&mut self, literal: &str, quote: Option<char>) {
        if literal.contains('\n') {
            self.push_multiline_literal(literal);
            return;
        }
        if quote.is_none()
            && self.layout.previous == PreviousToken::Operator
            && self.current.trimmed_end().ends_with_any(b"+-")
        {
            let before_sign = self.current.trim_end_matches([' ', '\t', '+', '-']);
            let cast_type = before_sign
                .strip_suffix(')')
                .and_then(|head| head.rsplit_once('('))
                .filter(|(before_open, ty)| {
                    !matches!(
                        trailing_word(before_open.trimmed_end()),
                        "sizeof" | "alignof" | "_Alignof"
                    ) && is_type_like_pointer_word(ty.trimmed())
                });
            // astyle keeps a space the source put after the sign.
            if cast_type.is_some()
                && self
                    .token_input
                    .previous_input_whitespace
                    .as_ref()
                    .is_none_or(|ws| ws.is_empty())
            {
                self.trim_current_end();
            }
        }
        if quote.is_none()
            && self.current.trimmed_end().ends_with('}')
            && self
                .token_input
                .previous_input_whitespace
                .as_ref()
                .is_some_and(|whitespace| !whitespace.is_empty())
        {
            self.emit_source_space();
        } else if self.current_ends_cast() {
            if self.options.pad_parens_outside || self.space_after_cast {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
        } else if self.layout.previous.needs_space_before_word() {
            if quote == Some('"')
                || (quote.is_none()
                    && literal.starts_with('.')
                    && matches!(
                        self.layout.previous,
                        PreviousToken::Word | PreviousToken::Literal
                    ))
            {
                self.emit_source_space();
            } else if matches!(
                self.layout.previous,
                PreviousToken::Word | PreviousToken::Literal
            ) {
                self.emit_source_space_or_ensure();
            } else if !self.previous_was_newline {
                self.emit_source_space();
            }
        }
        let string_continuation = quote.is_some().then(|| {
            let line_indent_spaces = self.current_line_indent_spaces();
            let has_stream_context = self.layout.frame_stack.active_stream().is_some()
                || self
                    .layout
                    .frame_stack
                    .string_continuation_before_output_line(self.output.len())
                    .is_some_and(|frame| frame.has_stream_context);
            StringContinuationFrame {
                output_line: self.output.len(),
                line_indent_spaces,
                literal_start_column: line_indent_spaces + self.current_visual_width(),
                line_starts_with_chain_operator: starts_with_chain_operator(
                    self.current.trimmed_start(),
                ),
                has_opening_context: self.current.last_open_paren().is_some(),
                has_open_brace_before_literal: self.current.holds_open_brace(),
                has_stream_context,
                inside_delimiter_context: self.layout.frame_stack.active_delimiter().is_some(),
            }
        });
        self.current.push_str(literal);
        if let Some(frame) = string_continuation {
            self.layout.frame_stack.set_string_continuation(frame);
        }
        if quote.is_some_and(|quote| !literal.ends_with(quote)) {
            self.layout.literal_line.unterminated_literal_line = true;
        }
        self.layout.command_state.observe_text(literal);
        self.layout.previous = PreviousToken::Literal;
        self.previous_was_newline = false;
    }

    fn push_multiline_literal(&mut self, literal: &str) {
        let unterminated_raw_literal = raw_literal_is_unterminated(literal);
        if self.layout.previous.needs_space_before_word() {
            self.emit_source_space();
        }
        let mut lines = literal.split('\n').peekable();
        if let Some(first) = lines.next() {
            self.current.push_str(first);
            self.layout.literal_line.preserve_raw_literal_line_end = true;
            self.finish_line();
        }
        while let Some(line) = lines.next() {
            if lines.peek().is_some() {
                self.adjust_and_publish_raw_literal_line(line.to_string(), line.len());
            } else if !line.is_empty() || !literal.ends_with('\n') {
                self.current.push_str(line);
                self.current_is_preindented = true;
                self.layout.literal_line.is_multiline_literal = true;
                self.layout.literal_line.multiline_literal_end = Some(self.current.len());
                self.layout.literal_line.unterminated_raw_literal = unterminated_raw_literal;
            }
        }
        self.layout.previous = PreviousToken::Literal;
        self.previous_was_newline = false;
    }
}
