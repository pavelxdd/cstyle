use crate::config::{FormatOptions, ObjCColonPad};
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, next_non_whitespace, token_text, tokenize};
use crate::formatter::state::frame::{BracketFrame, BracketRole};
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::has_unclosed_delimiter_after;
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::is_identifier_continue;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct ObjCLineState {
    pub(crate) post_prefix: bool,
    pub(crate) post_method_colon: bool,
    pub(crate) return_paren_depth: Option<usize>,
    pub(crate) param_paren_depth: Option<usize>,
    pub(crate) after_paren_pad: Option<bool>,
    pub(crate) colon_align: Option<usize>,
    pub(crate) method_continuation: bool,
    pub(crate) message_active: bool,
    pub(crate) message_pending_align: bool,
    pub(crate) message_align: Option<usize>,
}

fn line_is_label_style_dictionary_key(line: &str) -> bool {
    let trimmed = line.trimmed_end();
    let Some(before) = trimmed.strip_suffix(':') else {
        return false;
    };
    if !before.ends_with(char::is_whitespace) {
        return false;
    }
    let key = before.trimmed_end();
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn objc_message_selector_indent_spaces(
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
                let before = line[..offset].trimmed_end();
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

fn objc_method_colon_position(line: &str) -> Option<usize> {
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

pub(crate) fn objc_message_following_keyword_column(line: &str) -> Option<usize> {
    let chars: Vec<char> = line.chars().collect();
    let is_space = |ch: char| ch == ' ' || ch == '\t';
    let mut open_brackets = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, &ch) in chars.iter().enumerate() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
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

pub(crate) struct ObjCLineAlignment {
    pub(crate) indent_level: usize,
    pub(crate) exact_indent_spaces: Option<usize>,
    pub(crate) restore_message_align: Option<usize>,
}

impl FormatEngine<'_> {
    pub(crate) fn objc_dictionary_indent_spaces(
        &self,
        line: &str,
        mut current: Option<usize>,
    ) -> Option<usize> {
        if line.trimmed_start().starts_with("};")
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .take(64)
                .take_while(|line| !line.contains("@interface"))
                .any(|line| line.contains("@ {"))
        {
            let label_style_dictionary = self
                .output
                .scoped()
                .iter()
                .rev()
                .take(64)
                .take_while(|line| !line.contains("@ {"))
                .any(|line| line_is_label_style_dictionary_key(line));
            current = if label_style_dictionary {
                self.output
                    .scoped()
                    .iter()
                    .rev()
                    .take(64)
                    .find(|line| line.contains("@ {"))
                    .map(|opener| leading_visual_width(opener, self.options.tab_width))
            } else {
                self.layout
                    .previous_pre_adjust_line
                    .as_ref()
                    .map(|previous| {
                        leading_visual_width(previous, self.options.tab_width).saturating_sub(2)
                    })
            };
        }
        if !line.trimmed_start().starts_with('}')
            && !line_is_label_style_dictionary_key(line)
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| line_is_label_style_dictionary_key(previous))
            && let Some(opener) = self
                .output
                .scoped()
                .iter()
                .rev()
                .take(64)
                .find(|line| line.contains("@ {"))
        {
            current = Some(
                leading_visual_width(opener, self.options.tab_width) + self.options.indent_width,
            );
        }
        if !line.trimmed_start().starts_with('}')
            && !line_is_label_style_dictionary_key(line)
            && self
                .layout
                .previous_pre_adjust_line
                .as_ref()
                .is_some_and(|previous| previous.trimmed_end().ends_with(','))
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .take(64)
                .take_while(|line| !line.trimmed_end().ends_with("};"))
                .any(|line| line.contains("@ {"))
        {
            current = self
                .layout
                .previous_pre_adjust_line
                .as_ref()
                .map(|previous| leading_visual_width(previous, self.options.tab_width));
        }
        if line.trimmed_start().starts_with("@ {")
            && self
                .layout
                .previous_pre_adjust_line
                .as_ref()
                .is_some_and(|previous| previous.trimmed_end().ends_with('='))
        {
            current = self
                .layout
                .previous_pre_adjust_line
                .as_ref()
                .map(|previous| {
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width
                });
        }
        current
    }

    pub(crate) fn record_closed_objc_message_indent(
        &mut self,
        line: &str,
        closed_brackets: &[BracketFrame],
    ) {
        let indent_spaces = closed_brackets
            .iter()
            .rev()
            .find(|frame| {
                frame.role == BracketRole::ObjCMessage && frame.parent_objc_message_align.is_none()
            })
            .and_then(|frame| {
                objc_message_selector_indent_spaces(line, frame, self.options.tab_width)
            });
        self.max_length_line
            .set_objc_message_indent_spaces(indent_spaces);
    }

    pub(crate) fn apply_objc_message_alignment(
        &mut self,
        line: &str,
        closed_brackets: &[BracketFrame],
        mut indent_level: usize,
        mut exact_indent_spaces: Option<usize>,
    ) -> ObjCLineAlignment {
        if self
            .layout
            .previous_pre_adjust_line
            .as_deref()
            .is_some_and(|previous| previous.trimmed_end().ends_with('{'))
        {
            self.layout.objc.colon_align = None;
        }
        let closed_nested_bracket = closed_brackets.iter().find(|frame| {
            frame.parent_objc_message_align.is_some()
                && frame.opener_output_line < self.output.len()
        });
        let nested_message_align = closed_nested_bracket
            .and_then(|frame| frame.objc_continuation_indent_column())
            .or_else(|| {
                self.layout
                    .frame_stack
                    .active_bracket()
                    .filter(|frame| frame.opener_output_line < self.output.len())
                    .and_then(|frame| frame.objc_continuation_indent_column())
            });
        if let Some(spaces) = nested_message_align {
            self.layout.objc.message_align = Some(spaces);
        }
        let restore_message_align =
            closed_nested_bracket.and_then(|frame| frame.parent_objc_message_align);
        let mut force_message_align = false;
        if let Some(previous) = &self.layout.previous_pre_adjust_line {
            let previous_text = previous.trimmed_start();
            let line_text = line.trimmed_start();
            let simple_selector_line = line_text.split_once(':').is_some_and(|(key, rest)| {
                !rest.contains('?')
                    && key
                        .chars()
                        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            });
            let follows_nested_type_argument = previous_text
                .split_once(':')
                .is_some_and(|(key, _)| key.trimmed() == "type")
                && previous_text.ends_with(']')
                && !previous_text.starts_with('[');
            let follows_simple_selector = previous_text.split_once(':').is_some_and(|(key, _)| {
                key.chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            }) && !previous_text.starts_with('[');
            if simple_selector_line && follows_nested_type_argument {
                let spaces = self
                    .output
                    .last_line_outside_comment()
                    .map(|line| leading_visual_width(line, self.options.tab_width))
                    .unwrap_or_else(|| leading_visual_width(previous, self.options.tab_width))
                    .saturating_sub(1);
                exact_indent_spaces = Some(spaces);
                self.layout.objc.message_align = Some(spaces);
                force_message_align = true;
            } else if simple_selector_line
                && follows_simple_selector
                && self.layout.objc.message_align.is_some()
            {
                force_message_align = true;
            }
        }
        if self.layout.objc.message_pending_align {
            let base = exact_indent_spaces.unwrap_or_else(|| {
                ContinuationIndent::Level(indent_level).columns(self.options.indent_width)
            });
            if self.options.align_method_colon {
                if let Some(colon) = objc_method_colon_position(line) {
                    self.layout.objc.colon_align = Some(base + colon);
                    self.layout.objc.message_pending_align = false;
                } else if !self.layout.objc.message_active {
                    self.layout.objc.message_pending_align = false;
                }
            } else {
                self.layout.objc.message_pending_align = false;
                self.layout.objc.message_align =
                    objc_message_following_keyword_column(line).map(|column| base + column);
                if !self.layout.objc.message_active {
                    self.layout.objc.message_align = None;
                }
            }
        } else if let Some(align) = self.layout.objc.message_align
            && !line.trimmed_start().starts_with(['{', '}'])
        {
            let natural = exact_indent_spaces.unwrap_or_else(|| {
                ContinuationIndent::Level(indent_level).columns(self.options.indent_width)
            });
            exact_indent_spaces = Some(if force_message_align {
                align
            } else {
                natural.max(align)
            });
            if !self.layout.objc.message_active {
                self.layout.objc.message_align = None;
            }
        }
        if let Some(align_column) = self.layout.objc.colon_align {
            let first = line.chars().next();
            if matches!(first, Some('{') | Some('}') | Some('@')) {
                self.layout.objc.colon_align = None;
            } else if !matches!(first, Some('-') | Some('+'))
                && let Some(colon) = objc_method_colon_position(line)
                && colon <= align_column
            {
                exact_indent_spaces = Some(align_column - colon);
            }
            let trimmed_end = line.trimmed_end();
            if trimmed_end.ends_with(';') || trimmed_end.ends_with('{') {
                self.layout.objc.colon_align = None;
            }
        }
        if line.trimmed_start().starts_with('{')
            && self.output.last_line_outside_comment().is_some_and(|line| {
                line.trimmed_start()
                    .strip_prefix(['-', '+'])
                    .is_some_and(|rest| rest.trimmed_start().starts_with('('))
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

    pub(crate) fn restore_objc_message_alignment(&mut self, spaces: Option<usize>) {
        if let Some(spaces) = spaces {
            self.layout.objc.message_align = self
                .layout
                .frame_stack
                .has_objc_alignment_bracket()
                .then_some(spaces);
        }
    }

    pub(crate) fn objc_line_indent_override(&self, line: &str) -> Option<usize> {
        let mut spaces = None;
        if let Some(header) = ["@try", "@catch", "@finally"].into_iter().find(|header| {
            line.trimmed_start()
                .strip_prefix(header)
                .is_some_and(|rest| {
                    rest.is_empty()
                        || rest.starts_with(char::is_whitespace)
                        || rest.starts_with(['(', '{'])
                })
        }) {
            let active = self.layout.frame_stack.active_brace();
            let owner = if active.is_some_and(|frame| frame.header.as_deref() == Some(header)) {
                self.layout.frame_stack.enclosing_brace()
            } else {
                active
            };
            spaces = Some(owner.map_or(0, |frame| frame.body_indent_column));
        }

        let trimmed = line.trimmed_start();
        let interface_member = trimmed
            .strip_prefix(['+', '-'])
            .is_some_and(|rest| rest.trimmed_start().starts_with('('))
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
                .scoped()
                .iter()
                .rev()
                .take_while(|line| !line.trimmed_start().starts_with("@end"))
                .any(|line| line.trimmed_start().starts_with("@interface"))
        {
            spaces = Some(0);
        }
        spaces
    }

    pub(crate) fn ready_objc_method_closing_brace_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
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

    pub(crate) fn output_ends_objc_method_header(&self) -> bool {
        self.output_objc_method_header_indent_spaces().is_some()
    }

    pub(crate) fn output_objc_method_header_indent_spaces(&self) -> Option<usize> {
        for line in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
            .take(64)
        {
            let code = &self.output.code_of(line);
            if code.trimmed_end().ends_with([';', '{', '}']) {
                return None;
            }
            if code
                .trimmed_start()
                .strip_prefix(['+', '-'])
                .is_some_and(|rest| rest.trimmed_start().starts_with('('))
            {
                return Some(leading_visual_width(line, self.options.tab_width));
            }
        }
        None
    }

    pub(crate) fn is_objc_selector_or_message_colon(&self) -> bool {
        let current = self.current.trimmed_end();
        self.layout.frame_stack.bracket_depth() > 0
            || self.layout.objc.method_continuation
            || self.is_objc_method_line()
            || has_unclosed_delimiter_after(current, "[", "]")
            || has_unclosed_delimiter_after(current, "@selector(", ")")
    }

    pub(crate) fn is_objc_method_line(&self) -> bool {
        if self.layout.nesting.paren_depth > 0 {
            return false;
        }
        let line = self.current.trimmed_start();
        line.strip_prefix(['-', '+'])
            .is_some_and(|rest| rest.trimmed_start().starts_with('('))
    }

    pub(crate) fn is_objc_method_prefix(&self, next: Option<&Token>) -> bool {
        matches!(next, Some(Token::Symbol('('))) && self.current.trimmed().is_empty()
    }

    pub(crate) fn compute_objc_method_colon_align(
        &self,
        tokens: &[Token],
        start: usize,
    ) -> Option<usize> {
        let base = ContinuationIndent::Level(self.layout.indentation.indent())
            .columns(self.options.indent_width);
        let cont_indent = ContinuationIndent::Level(self.layout.indentation.indent() + 1)
            .columns(self.options.indent_width);

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
            + objc_method_first_colon_output_column(self.options, tokens, start)
                .or(line_colons[0])?;
        let max_continuation = line_colons[1..].iter().filter_map(|pos| *pos).max()?;
        Some(first_colon.max(cont_indent + max_continuation))
    }

    pub(crate) fn is_objc_standalone_line(&self) -> bool {
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

pub(crate) fn token_starts_objc_method_definition(
    tokens: &[Token],
    index: usize,
    line_end: usize,
) -> bool {
    matches!(&tokens[index], Token::Operator(op) if op == "-" || op == "+")
        && next_non_whitespace(tokens, index + 1, line_end)
            .is_some_and(|next| matches!(tokens[next], Token::Symbol('(')))
}

fn objc_method_first_colon_output_column(
    options: &FormatOptions,
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
    let selector_end = source[..colon].trimmed_end().len();
    if selector_start > selector_end {
        return None;
    }

    let prefix_gap = &source[1..open];
    let prefix_gap = if options.pad_method_prefix {
        " "
    } else if options.unpad_method_prefix {
        ""
    } else {
        prefix_gap
    };
    let return_gap = &source[close + 1..selector_start];
    let return_gap = if options.pad_return_type {
        " "
    } else if options.unpad_return_type {
        ""
    } else {
        return_gap
    };
    let colon_gap = &source[selector_end..colon];
    let colon_gap = match options.pad_method_colon {
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
    Some(visual_width_from(&prefix, 0, options.tab_width))
}
