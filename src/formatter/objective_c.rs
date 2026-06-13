use super::columns::{leading_visual_width, visual_width_from};
use super::frame::{BracketFrame, BracketRole};
use super::line_scan::has_unclosed_delimiter_after;
use super::state::ContinuationIndent;
use super::token::{Token, next_non_whitespace, token_text, tokenize};
use super::{FormatEngine, is_identifier_continue, trailing_comment_split_limit};
use crate::config::ObjCColonPad;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct ObjectiveCLineState {
    pub(super) post_prefix: bool,
    pub(super) post_method_colon: bool,
    pub(super) return_paren_depth: Option<usize>,
    pub(super) param_paren_depth: Option<usize>,
    pub(super) after_paren_pad: Option<bool>,
    pub(super) colon_align: Option<usize>,
    pub(super) method_continuation: bool,
    pub(super) message_active: bool,
    pub(super) message_pending_align: bool,
    pub(super) message_align: Option<usize>,
}

fn line_is_label_style_dictionary_key(line: &str) -> bool {
    let trimmed = line.trim_end();
    let Some(before) = trimmed.strip_suffix(':') else {
        return false;
    };
    if !before.ends_with(char::is_whitespace) {
        return false;
    }
    let key = before.trim_end();
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

pub(super) fn objc_message_selector_indent_spaces(
    line: &str,
    frame: &BracketFrame,
    tab_width: usize,
) -> Option<usize> {
    let opener_column = frame
        .opener_output_column
        .saturating_sub(frame.line_indent_spaces);
    let mut offset = 0usize;
    let mut depth = 0usize;
    let mut in_message = false;
    for token in tokenize(line) {
        let text = token_text(&token);
        match token {
            Token::Symbol('[')
                if !in_message
                    && visual_width_from(&line[..offset], 0, tab_width) == opener_column =>
            {
                in_message = true;
                depth = 1;
            }
            Token::Symbol('[') if in_message => depth += 1,
            Token::Symbol(']') if in_message => depth = depth.saturating_sub(1),
            Token::Symbol(':') if in_message && depth == 1 => {
                let before = line[..offset].trim_end();
                let selector_len = before
                    .chars()
                    .rev()
                    .take_while(|ch| is_identifier_continue(*ch))
                    .map(char::len_utf8)
                    .sum::<usize>();
                if selector_len == 0 {
                    return None;
                }
                let selector_start = before.len() - selector_len;
                return Some(
                    frame.line_indent_spaces
                        + visual_width_from(
                            &line[..selector_start],
                            frame.line_indent_spaces,
                            tab_width,
                        ),
                );
            }
            _ => {}
        }
        offset += text.len();
    }
    None
}

pub(super) fn objc_method_colon_position(line: &str) -> Option<usize> {
    let mut ternary = false;
    for (index, ch) in line.chars().enumerate() {
        match ch {
            '?' => ternary = true,
            ':' if ternary => ternary = false,
            ':' => return Some(index),
            _ => {}
        }
    }
    None
}

pub(super) fn objc_message_following_keyword_column(line: &str) -> Option<usize> {
    let chars: Vec<char> = line.chars().collect();
    let is_space = |ch: char| ch == ' ' || ch == '\t';
    let mut open_brackets = Vec::new();
    for (index, ch) in chars.iter().enumerate() {
        match ch {
            '[' => open_brackets.push(index),
            ']' => {
                open_brackets.pop();
            }
            _ => {}
        }
    }
    let nested_message = open_brackets.len() > 1;
    let bracket = open_brackets.last().copied()?;
    if nested_message {
        return Some(bracket + 1);
    }
    let first_text = (bracket + 1..chars.len()).find(|&index| !is_space(chars[index]))?;
    let object_end = if chars[first_text] == '[' {
        match (first_text + 1..chars.len()).find(|&index| chars[index] == ']') {
            Some(end) => end,
            None => return Some(bracket + 1),
        }
    } else {
        let search_start = if chars[first_text] == '(' {
            match (first_text + 1..chars.len()).find(|&index| chars[index] == ')') {
                Some(end) => end,
                None => return Some(bracket + 1),
            }
        } else {
            first_text
        };
        match (search_start + 1..chars.len()).find(|&index| is_space(chars[index])) {
            Some(end) => end - 1,
            None => return Some(bracket + 1),
        }
    };
    match (object_end + 1..chars.len()).find(|&index| !is_space(chars[index])) {
        Some(keyword) => Some(keyword),
        None => Some(bracket + 1),
    }
}

pub(super) struct ObjCLineAlignment {
    pub(super) indent_level: usize,
    pub(super) exact_indent_spaces: Option<usize>,
    pub(super) restore_message_align: Option<usize>,
}

impl FormatEngine<'_> {
    pub(super) fn objc_dictionary_indent_spaces(
        &self,
        line: &str,
        mut current: Option<usize>,
    ) -> Option<usize> {
        if line.trim_start().starts_with("};")
            && self
                .output
                .iter()
                .rev()
                .take(64)
                .take_while(|line| !line.contains("@interface"))
                .any(|line| line.contains("@ {"))
        {
            let label_style_dictionary = self
                .output
                .iter()
                .rev()
                .take(64)
                .take_while(|line| !line.contains("@ {"))
                .any(|line| line_is_label_style_dictionary_key(line));
            current = if label_style_dictionary {
                self.output
                    .iter()
                    .rev()
                    .take(64)
                    .find(|line| line.contains("@ {"))
                    .map(|opener| leading_visual_width(opener, self.options.tab_width))
            } else {
                self.previous_pre_adjust_line.as_ref().map(|previous| {
                    leading_visual_width(previous, self.options.tab_width).saturating_sub(2)
                })
            };
        }
        if !line.trim_start().starts_with('}')
            && !line_is_label_style_dictionary_key(line)
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| line_is_label_style_dictionary_key(previous))
            && let Some(opener) = self
                .output
                .iter()
                .rev()
                .take(64)
                .find(|line| line.contains("@ {"))
        {
            current = Some(
                leading_visual_width(opener, self.options.tab_width) + self.options.indent_width,
            );
        }
        if !line.trim_start().starts_with('}')
            && !line_is_label_style_dictionary_key(line)
            && self
                .previous_pre_adjust_line
                .as_ref()
                .is_some_and(|previous| previous.trim_end().ends_with(','))
            && self
                .output
                .iter()
                .rev()
                .take(64)
                .take_while(|line| !line.trim_end().ends_with("};"))
                .any(|line| line.contains("@ {"))
        {
            current = self
                .previous_pre_adjust_line
                .as_ref()
                .map(|previous| leading_visual_width(previous, self.options.tab_width));
        }
        if line.trim_start().starts_with("@ {")
            && self
                .previous_pre_adjust_line
                .as_ref()
                .is_some_and(|previous| previous.trim_end().ends_with('='))
        {
            current = self.previous_pre_adjust_line.as_ref().map(|previous| {
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width
            });
        }
        current
    }

    pub(super) fn record_closed_objc_message_indent(
        &mut self,
        line: &str,
        closed_brackets: &[BracketFrame],
    ) {
        let indent_spaces = closed_brackets
            .iter()
            .rev()
            .find(|frame| {
                frame.role == BracketRole::ObjectiveCMessage
                    && frame.parent_objc_message_align.is_none()
            })
            .and_then(|frame| {
                objc_message_selector_indent_spaces(line, frame, self.options.tab_width)
            });
        self.max_length_line
            .set_objc_message_indent_spaces(indent_spaces);
    }

    pub(super) fn apply_objc_message_alignment(
        &mut self,
        line: &str,
        closed_brackets: &[BracketFrame],
        mut indent_level: usize,
        mut exact_indent_spaces: Option<usize>,
    ) -> ObjCLineAlignment {
        if self
            .previous_pre_adjust_line
            .as_deref()
            .is_some_and(|previous| previous.trim_end().ends_with('{'))
        {
            self.objc.colon_align = None;
        }
        let closed_nested_bracket = closed_brackets.iter().find(|frame| {
            frame.parent_objc_message_align.is_some()
                && frame.opener_output_line < self.output.len()
        });
        let nested_message_align = closed_nested_bracket
            .and_then(|frame| frame.objc_continuation_indent_column())
            .or_else(|| {
                self.frame_stack
                    .active_bracket()
                    .filter(|frame| frame.opener_output_line < self.output.len())
                    .and_then(|frame| frame.objc_continuation_indent_column())
            });
        if let Some(spaces) = nested_message_align {
            self.objc.message_align = Some(spaces);
        }
        let restore_message_align =
            closed_nested_bracket.and_then(|frame| frame.parent_objc_message_align);
        let mut force_message_align = false;
        if let Some(previous) = &self.previous_pre_adjust_line {
            let previous_text = previous.trim_start();
            let line_text = line.trim_start();
            let simple_selector_line = line_text.split_once(':').is_some_and(|(key, rest)| {
                !rest.contains('?')
                    && key
                        .chars()
                        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            });
            let follows_nested_type_argument = previous_text
                .split_once(':')
                .is_some_and(|(key, _)| key.trim() == "type")
                && previous_text.ends_with(']')
                && !previous_text.starts_with('[');
            let follows_simple_selector = previous_text.split_once(':').is_some_and(|(key, _)| {
                key.chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            }) && !previous_text.starts_with('[');
            if simple_selector_line && follows_nested_type_argument {
                let spaces = self
                    .output
                    .last_non_empty_line()
                    .map(|line| leading_visual_width(line, self.options.tab_width))
                    .unwrap_or_else(|| leading_visual_width(previous, self.options.tab_width))
                    .saturating_sub(1);
                exact_indent_spaces = Some(spaces);
                self.objc.message_align = Some(spaces);
                force_message_align = true;
            } else if simple_selector_line
                && follows_simple_selector
                && self.objc.message_align.is_some()
            {
                force_message_align = true;
            }
        }
        if self.objc.message_pending_align {
            let base = exact_indent_spaces.unwrap_or_else(|| {
                ContinuationIndent::Level(indent_level).columns(self.options.indent_width)
            });
            if self.options.align_method_colon {
                if let Some(colon) = objc_method_colon_position(line) {
                    self.objc.colon_align = Some(base + colon);
                    self.objc.message_pending_align = false;
                } else if !self.objc.message_active {
                    self.objc.message_pending_align = false;
                }
            } else {
                self.objc.message_pending_align = false;
                self.objc.message_align =
                    objc_message_following_keyword_column(line).map(|column| base + column);
                if !self.objc.message_active {
                    self.objc.message_align = None;
                }
            }
        } else if let Some(align) = self.objc.message_align
            && !line.trim_start().starts_with(['{', '}'])
        {
            let natural = exact_indent_spaces.unwrap_or_else(|| {
                ContinuationIndent::Level(indent_level).columns(self.options.indent_width)
            });
            exact_indent_spaces = Some(if force_message_align {
                align
            } else {
                natural.max(align)
            });
            if !self.objc.message_active {
                self.objc.message_align = None;
            }
        }
        if let Some(align_column) = self.objc.colon_align {
            let first = line.chars().next();
            if matches!(first, Some('{') | Some('}') | Some('@')) {
                self.objc.colon_align = None;
            } else if !matches!(first, Some('-') | Some('+'))
                && let Some(colon) = objc_method_colon_position(line)
                && colon <= align_column
            {
                exact_indent_spaces = Some(align_column - colon);
            }
            let trimmed_end = line.trim_end();
            if trimmed_end.ends_with(';') || trimmed_end.ends_with('{') {
                self.objc.colon_align = None;
            }
        }
        if line.trim_start().starts_with('{')
            && self.output.last_non_empty_line().is_some_and(|line| {
                line.trim_start()
                    .strip_prefix(['-', '+'])
                    .is_some_and(|rest| rest.trim_start().starts_with('('))
            })
        {
            indent_level = 0;
            exact_indent_spaces = None;
        }
        ObjCLineAlignment {
            indent_level,
            exact_indent_spaces,
            restore_message_align,
        }
    }

    pub(super) fn restore_objc_message_alignment(&mut self, spaces: Option<usize>) {
        if let Some(spaces) = spaces {
            self.objc.message_align = self
                .frame_stack
                .has_objc_alignment_bracket()
                .then_some(spaces);
        }
    }

    pub(super) fn objc_line_indent_override(&self, line: &str) -> Option<usize> {
        let mut spaces = None;
        if let Some(header) = ["@try", "@catch", "@finally"].into_iter().find(|header| {
            line.trim_start().strip_prefix(header).is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with(char::is_whitespace)
                    || rest.starts_with(['(', '{'])
            })
        }) {
            let active = self.frame_stack.active_brace();
            let owner = if active.is_some_and(|frame| frame.header.as_deref() == Some(header)) {
                self.frame_stack.enclosing_brace()
            } else {
                active
            };
            spaces = Some(owner.map_or(0, |frame| frame.body_indent_column));
        }

        let trimmed = line.trim_start();
        let interface_member = trimmed
            .strip_prefix(['+', '-'])
            .is_some_and(|rest| rest.trim_start().starts_with('('))
            || ["@property", "@required", "@optional"]
                .into_iter()
                .any(|word| {
                    trimmed.strip_prefix(word).is_some_and(|rest| {
                        rest.is_empty()
                            || rest.starts_with(char::is_whitespace)
                            || rest.starts_with('(')
                    })
                });
        if interface_member
            && self
                .output
                .iter()
                .rev()
                .take_while(|line| !line.trim_start().starts_with("@end"))
                .any(|line| line.trim_start().starts_with("@interface"))
        {
            spaces = Some(0);
        }
        spaces
    }

    pub(super) fn ready_objc_method_closing_brace_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let line_start = line.trim_start();
        if line_start != "}" {
            return None;
        }
        let previous_index = (0..self.output.len())
            .rev()
            .find(|&index| !self.output.trimmed(index).is_empty())?;
        (self.output.code(previous_index).ends_with("];")
            && (0..self.output.len())
                .rev()
                .take(16)
                .any(|index| self.output.trimmed(index).starts_with("- (")))
        .then_some(0)
    }

    pub(super) fn output_ends_objc_method_header(&self) -> bool {
        self.output_objc_method_header_indent_spaces().is_some()
    }

    pub(super) fn output_objc_method_header_indent_spaces(&self) -> Option<usize> {
        for line in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(64)
        {
            let code = &line[..trailing_comment_split_limit(line)];
            if code.trim_end().ends_with([';', '{', '}']) {
                return None;
            }
            if code
                .trim_start()
                .strip_prefix(['+', '-'])
                .is_some_and(|rest| rest.trim_start().starts_with('('))
            {
                return Some(leading_visual_width(line, self.options.tab_width));
            }
        }
        None
    }

    pub(super) fn is_objc_selector_or_message_colon(&self) -> bool {
        let current = self.current.trim_end();
        self.frame_stack.bracket_depth() > 0
            || self.objc.method_continuation
            || self.is_objc_method_line()
            || has_unclosed_delimiter_after(current, "[", "]")
            || has_unclosed_delimiter_after(current, "@selector(", ")")
    }

    pub(super) fn is_objc_method_line(&self) -> bool {
        if self.stack_state.paren_depth > 0 {
            return false;
        }
        let line = self.current.trim_start();
        line.strip_prefix(['-', '+'])
            .is_some_and(|rest| rest.trim_start().starts_with('('))
    }

    pub(super) fn is_objc_method_prefix(&self, next: Option<&Token>) -> bool {
        matches!(next, Some(Token::Symbol('('))) && self.current.trim().is_empty()
    }

    pub(super) fn token_starts_objc_method_definition(
        &self,
        tokens: &[Token],
        index: usize,
        line_end: usize,
    ) -> bool {
        matches!(&tokens[index], Token::Operator(op) if op == "-" || op == "+")
            && next_non_whitespace(tokens, index + 1, line_end)
                .is_some_and(|next| matches!(tokens[next], Token::Symbol('(')))
    }

    pub(super) fn compute_objc_method_colon_align(
        &self,
        tokens: &[Token],
        start: usize,
    ) -> Option<usize> {
        let base =
            ContinuationIndent::Level(self.state.indent()).columns(self.options.indent_width);
        let cont_indent =
            ContinuationIndent::Level(self.state.indent() + 1).columns(self.options.indent_width);

        let mut line_colons: Vec<Option<usize>> = Vec::new();
        let mut column = 0usize;
        let mut last_token_end = 0usize;
        let mut started = false;
        let mut colon: Option<usize> = None;
        let mut ternary = false;
        let mut ended = false;

        for token in &tokens[start..] {
            match token {
                Token::Newline => {
                    line_colons.push(colon);
                    column = 0;
                    last_token_end = 0;
                    started = false;
                    colon = None;
                    ternary = false;
                }
                Token::Whitespace(ws) => {
                    if started {
                        column += ws.chars().count();
                    }
                }
                other => {
                    started = true;
                    let text = token_text(other);
                    if matches!(other, Token::Symbol('{') | Token::Symbol(';')) {
                        ended = true;
                    } else if matches!(other, Token::Symbol('?')) {
                        ternary = true;
                    } else if matches!(other, Token::Symbol(':')) && colon.is_none() {
                        if ternary {
                            ternary = false;
                        } else {
                            colon = Some(match self.options.pad_method_colon {
                                ObjCColonPad::NoChange => column,
                                ObjCColonPad::All | ObjCColonPad::Before => last_token_end + 1,
                                ObjCColonPad::None | ObjCColonPad::After => last_token_end,
                            });
                        }
                    }
                    column += text.chars().count();
                    last_token_end = column;
                }
            }
            if ended {
                line_colons.push(colon);
                break;
            }
        }
        if !ended {
            line_colons.push(colon);
        }

        if line_colons.len() < 2 {
            return None;
        }
        let first_colon = base
            + self
                .objc_method_first_colon_output_column(tokens, start)
                .or(line_colons[0])?;
        let max_continuation = line_colons[1..].iter().filter_map(|pos| *pos).max()?;
        Some(first_colon.max(cont_indent + max_continuation))
    }

    fn objc_method_first_colon_output_column(
        &self,
        tokens: &[Token],
        start: usize,
    ) -> Option<usize> {
        let source = tokens[start..]
            .iter()
            .take_while(|token| !matches!(token, Token::Newline))
            .map(token_text)
            .collect::<String>();
        if !source.starts_with(['-', '+']) {
            return None;
        }
        let open = source.find('(')?;
        let mut depth = 0usize;
        let mut close = None;
        for (offset, ch) in source[open..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        close = Some(open + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close?;
        let colon = close + 1 + source[close + 1..].find(':')?;
        let selector_start =
            close + 1 + source[close + 1..colon].find(|ch: char| !ch.is_whitespace())?;
        let selector_end = source[..colon].trim_end().len();
        if selector_start > selector_end {
            return None;
        }

        let prefix_gap = &source[1..open];
        let prefix_gap = if self.options.pad_method_prefix {
            " "
        } else if self.options.unpad_method_prefix {
            ""
        } else {
            prefix_gap
        };
        let return_gap = &source[close + 1..selector_start];
        let return_gap = if self.options.pad_return_type {
            " "
        } else if self.options.unpad_return_type {
            ""
        } else {
            return_gap
        };
        let colon_gap = &source[selector_end..colon];
        let colon_gap = match self.options.pad_method_colon {
            ObjCColonPad::NoChange => colon_gap,
            ObjCColonPad::All | ObjCColonPad::Before => " ",
            ObjCColonPad::None | ObjCColonPad::After => "",
        };
        let prefix = format!(
            "{}{}{}{}{}{}",
            &source[..1],
            prefix_gap,
            &source[open..=close],
            return_gap,
            &source[selector_start..selector_end],
            colon_gap,
        );
        Some(visual_width_from(&prefix, 0, self.options.tab_width))
    }

    pub(super) fn is_objc_standalone_line(&self) -> bool {
        matches!(
            self.current.split_whitespace().next(),
            Some(
                "@interface"
                    | "@implementation"
                    | "@protocol"
                    | "@end"
                    | "@private"
                    | "@protected"
                    | "@public"
                    | "@package"
                    | "@optional"
                    | "@required"
            )
        )
    }
}
