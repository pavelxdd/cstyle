use super::frame::StringContinuationFrame;
use super::operators::starts_with_chain_operator;
use super::{
    FormatEngine, PreviousToken, is_type_like_pointer_word, trailing_comment_split_limit,
    trailing_word,
};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct LiteralLineState {
    pub(super) is_multiline_literal: bool,
    pub(super) multiline_literal_end: Option<usize>,
    pub(super) unterminated_raw_literal: bool,
    pub(super) preserve_raw_literal_line_end: bool,
    pub(super) unterminated_literal_line: bool,
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

pub(super) fn first_string_literal_start(line: &str) -> Option<usize> {
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
            || prefixes
                .iter()
                .any(|prefix| code[index..].starts_with(prefix))
        {
            return Some(index);
        }
    }
    None
}

pub(super) fn starts_string_literal_token(line: &str) -> bool {
    first_string_literal_start(line) == Some(0)
}

pub(super) fn string_literal_token_end(line: &str, start: usize) -> Option<usize> {
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

pub(super) fn string_literal_has_opening_context(line: &str, start: usize) -> bool {
    !matches!(
        line[..start].trim_end().chars().next_back(),
        Some(ch) if super::is_identifier_continue(ch) || matches!(ch, ')' | ']')
    )
}

pub(super) fn single_string_literal_comma_line(line: &str) -> bool {
    let Some(start) = first_string_literal_start(line) else {
        return false;
    };
    if !line[..start].trim().is_empty() {
        return false;
    }
    let Some(end) = string_literal_token_end(line, start) else {
        return false;
    };
    line[end..].trim() == ","
}

pub(super) fn last_string_literal_start(line: &str) -> Option<usize> {
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
    pub(super) fn try_finish_multiline_literal_line(&mut self) -> bool {
        if !self.literal_line.is_multiline_literal {
            return false;
        }
        let structural_start = self
            .literal_line
            .multiline_literal_end
            .take()
            .unwrap_or(self.current.len());
        let preserve_line_end =
            self.current.contains('\x0c') || self.literal_line.unterminated_raw_literal;
        let line = self.take_current();
        let line = if preserve_line_end {
            line
        } else {
            line.trim_end().to_string()
        };
        self.adjust_and_publish_raw_literal_line(line, structural_start);
        self.reset_after_finished_line();
        true
    }

    pub(super) fn push_literal(&mut self, literal: &str, quote: Option<char>) {
        if literal.contains('\n') {
            self.push_multiline_literal(literal);
            return;
        }
        if quote.is_none()
            && self.previous == PreviousToken::Operator
            && self.current.trim_end().ends_with(['+', '-'])
        {
            let before_sign = self.current.trim_end_matches([' ', '\t', '+', '-']);
            let cast_type = before_sign
                .strip_suffix(')')
                .and_then(|head| head.rsplit_once('('))
                .filter(|(before_open, ty)| {
                    !matches!(
                        trailing_word(before_open.trim_end()),
                        "sizeof" | "alignof" | "_Alignof"
                    ) && is_type_like_pointer_word(ty.trim())
                });
            if cast_type.is_some() {
                self.trim_current_end();
            }
        }
        if quote.is_none()
            && self.current.trim_end().ends_with('}')
            && self
                .token_input
                .previous_input_whitespace
                .as_ref()
                .is_some_and(|whitespace| !whitespace.is_empty())
        {
            self.emit_source_space();
        } else if self.current_ends_cast() {
            if self.options.pad_parens_outside || self.space_after_cast {
                self.ensure_space();
            } else {
                self.emit_source_space();
            }
        } else if self.previous.needs_space_before_word() {
            if quote == Some('"') {
                self.emit_source_space();
            } else if quote.is_none()
                && literal.starts_with('.')
                && matches!(self.previous, PreviousToken::Word | PreviousToken::Literal)
            {
                self.emit_source_space();
            } else if matches!(self.previous, PreviousToken::Word | PreviousToken::Literal) {
                self.emit_source_space_or_ensure();
            } else if !self.previous_was_newline {
                self.emit_source_space();
            }
        }
        let string_continuation = quote.is_some().then(|| {
            let line_indent_spaces = self.current_line_indent_spaces();
            let has_stream_context = self.frame_stack.active_stream().is_some()
                || self
                    .frame_stack
                    .string_continuation_before_output_line(self.output.len())
                    .is_some_and(|frame| frame.has_stream_context);
            StringContinuationFrame {
                output_line: self.output.len(),
                line_indent_spaces,
                literal_start_column: line_indent_spaces + self.current_visual_width(),
                line_starts_with_chain_operator: starts_with_chain_operator(
                    self.current.trim_start(),
                ),
                has_opening_context: self.current.contains('('),
                has_open_brace_before_literal: self.current.contains('{'),
                has_stream_context,
                inside_delimiter_context: self.frame_stack.active_delimiter().is_some(),
            }
        });
        self.current.push_str(literal);
        if let Some(frame) = string_continuation {
            self.frame_stack.set_string_continuation(frame);
        }
        if quote.is_some_and(|quote| !literal.ends_with(quote)) {
            self.literal_line.unterminated_literal_line = true;
        }
        self.command_state.observe_text(literal);
        self.previous = PreviousToken::Literal;
        self.previous_was_newline = false;
    }

    fn push_multiline_literal(&mut self, literal: &str) {
        let unterminated_raw_literal = raw_literal_is_unterminated(literal);
        if self.previous.needs_space_before_word() {
            self.emit_source_space();
        }
        let mut lines = literal.split('\n').peekable();
        if let Some(first) = lines.next() {
            self.current.push_str(first);
            self.literal_line.preserve_raw_literal_line_end = true;
            self.finish_line();
        }
        while let Some(line) = lines.next() {
            if lines.peek().is_some() {
                self.adjust_and_publish_raw_literal_line(line.to_string(), line.len());
            } else if !line.is_empty() || !literal.ends_with('\n') {
                self.current.push_str(line);
                self.current_is_preindented = true;
                self.literal_line.is_multiline_literal = true;
                self.literal_line.multiline_literal_end = Some(self.current.len());
                self.literal_line.unterminated_raw_literal = unterminated_raw_literal;
            }
        }
        self.previous = PreviousToken::Literal;
        self.previous_was_newline = false;
    }
}
