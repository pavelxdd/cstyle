use super::FormatEngine;

use super::brace_classification::is_lambda_capture_header;

use super::columns::{leading_visual_width, visual_width_from};
use super::compound_literals::line_ends_compound_literal_cast;
use super::frame::{BraceSemanticKind, ParenRole};
use super::headers::is_braceless_header_line;
use super::indentation::LineKind;

use super::language::is_macro_like_word;
use super::line_scan::{
    has_unmatched_open_brace, trailing_comment_split_limit, unmatched_open_paren_column,
};
use super::operators::{starts_ternary_arm, starts_with_chain_operator};
use super::preprocessor::{is_conditional_preprocessor, preprocessor_directive};

use super::state::{FormatterBraceType, InlineArrayFrame, PreviousToken};
use super::token::{Token, next_non_whitespace};
use crate::config::{BraceStyle, MinConditionalIndent};
use crate::source::lex::is_identifier_continue;

pub(super) struct CompoundLiteralOpeningLayout {
    pub(super) line_indent_spaces: usize,
    pub(super) brace_indent_spaces: usize,
}

fn line_opens_typed_initializer(line: &str) -> bool {
    let code = line[..trailing_comment_split_limit(line)].trim_end();
    let Some(open) = code.rfind('{') else {
        return false;
    };
    let before = code[..open].trim_end();
    before.contains('<') && before.chars().next_back() == Some('>')
}

pub(super) fn initializer_sibling_uses_previous_indent(line: &str) -> bool {
    if line.starts_with(['&', '{', '"', '\'']) || line.starts_with(|ch: char| ch.is_ascii_digit()) {
        return true;
    }
    let word_end = line
        .find(|ch: char| !is_identifier_continue(ch))
        .unwrap_or(line.len());
    word_end > 0
        && is_macro_like_word(&line[..word_end])
        && !line[word_end..].trim_start().starts_with('(')
}

pub(super) fn has_nested_designated_init_brace(tokens: &[Token]) -> bool {
    let mut saw_open = false;
    let mut saw_designator = false;
    for token in tokens {
        match token {
            Token::Whitespace(_) | Token::Newline => {}
            Token::Symbol('{') if saw_designator => return true,
            Token::Symbol('{') => saw_open = true,
            Token::Symbol('.') if saw_open => saw_designator = true,
            _ => {}
        }
    }
    false
}

pub(super) fn bracket_starts_initializer_designator(
    tokens: &[Token],
    start: usize,
    end: usize,
) -> bool {
    if !matches!(tokens.get(start), Some(Token::Symbol('['))) {
        return false;
    }
    let mut open = start;
    loop {
        let Some(close) = matching_close_bracket_on_line(tokens, open, end) else {
            return false;
        };
        let Some(next) = next_non_whitespace(tokens, close + 1, end) else {
            return false;
        };
        match tokens.get(next) {
            Some(Token::Symbol('[')) => open = next,
            Some(Token::Operator(operator)) if operator == "=" => return true,
            _ => return false,
        }
    }
}

fn matching_close_bracket_on_line(tokens: &[Token], open: usize, end: usize) -> Option<usize> {
    if !matches!(tokens.get(open), Some(Token::Symbol('['))) {
        return None;
    }
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().take(end).skip(open) {
        match token {
            Token::Symbol('[') => depth += 1,
            Token::Symbol(']') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

impl FormatEngine<'_> {
    pub(super) fn clear_macro_interrupted_initializer_frames(&mut self) {
        while self
            .inline_array
            .frames
            .last()
            .and_then(|frame| self.output.get(frame.output_line))
            .is_some_and(|line| line.contains('#') && !line.trim_start().starts_with('#'))
        {
            self.inline_array.frames.pop();
            if matches!(
                self.stack_state.brace_type_stack.last(),
                Some(
                    FormatterBraceType::Array
                        | FormatterBraceType::Init
                        | FormatterBraceType::CompoundLiteral
                )
            ) {
                self.exit_brace_state();
            }
        }
    }

    pub(super) fn designated_initializer_source_indent_spaces(&self, line: &str) -> Option<usize> {
        if self.options.min_conditional_indent != MinConditionalIndent::Zero {
            return None;
        }
        let trimmed = line.trim_start();
        let previous = self.output.last_non_empty_line();
        if !(self.in_initializer_brace()
            || self.in_aggregate_declaration_brace()
            || (trimmed.starts_with('[')
                && previous.is_some_and(|line| line.trim_start().starts_with('['))))
        {
            return None;
        }
        let recent_designator = self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take_while(|line| !line.trim_start().starts_with("},"))
            .any(|line| line.trim_start().starts_with('['));
        if !trimmed.starts_with('[')
            && !(recent_designator && (trimmed.starts_with('.') || trimmed.starts_with("},")))
        {
            return None;
        }
        if trimmed.starts_with('[')
            && self.token_input.input_source_indent == 0
            && let Some(previous) = previous
            && previous.trim_start().starts_with('[')
            && previous.trim_end().ends_with(',')
        {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        Some(self.token_input.input_source_indent)
    }

    pub(super) fn range_designator_source_indent_spaces(&self, line: &str) -> Option<usize> {
        if self.options.min_conditional_indent != MinConditionalIndent::Zero
            || !line.trim_start().starts_with('[')
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if !previous.trim_start().starts_with('[') {
            return None;
        }
        if self.token_input.input_source_indent == 0 && line.trim_start().contains("...") {
            Some(leading_visual_width(previous, self.options.tab_width))
        } else {
            Some(self.token_input.input_source_indent)
        }
    }

    pub(super) fn designated_initializer_source_indent_floor(
        &self,
        line: &str,
        kind: LineKind,
        current: Option<usize>,
    ) -> Option<usize> {
        if kind != LineKind::Normal
            || !line.trim_start().starts_with('.')
            || !(self.in_initializer_brace()
                || self.output_has_open_initializer_brace()
                || self.current_inline_array_column().is_some())
        {
            return None;
        }
        let spaces = self.state.indent() * self.options.indent_width;
        (self.token_input.input_source_indent >= spaces)
            .then(|| current.map_or(spaces, |value| value.max(spaces)))
    }

    pub(super) fn recent_double_brace_indent_spaces(&self, line: &str) -> Option<usize> {
        let opening = self
            .output
            .iter()
            .rev()
            .take(4)
            .find(|previous| previous.trim_end().ends_with("{{"))?;
        let opening_indent = leading_visual_width(opening, self.options.tab_width);
        if !line.trim_start().starts_with(['{', '}']) {
            Some(opening_indent + self.options.indent_width * 2)
        } else if line.trim() == "}" {
            Some(opening_indent + self.options.indent_width)
        } else {
            None
        }
    }

    pub(super) fn closed_initializer_or_array_indent_spaces(
        &self,
        line: &str,
        indent: usize,
        normal_indent: usize,
    ) -> Option<usize> {
        if !matches!(
            self.stack_state.last_closed_brace_type,
            Some(
                FormatterBraceType::Array
                    | FormatterBraceType::CompoundLiteral
                    | FormatterBraceType::Init
            )
        ) {
            return None;
        }
        if line.trim_start().starts_with("}, ") {
            return Some(indent.max(self.continuation_base_indent()) * self.options.indent_width);
        }
        if line.trim() != "}"
            || !self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|previous| {
                    previous[..trailing_comment_split_limit(previous)]
                        .trim_end()
                        .ends_with(')')
                })
        {
            return None;
        }
        Some((normal_indent * self.options.indent_width).max(
            self.continuation_base_indent() * self.options.indent_width + self.options.indent_width,
        ))
    }

    pub(super) fn initializer_or_array_opening_brace_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if line.trim() != "{"
            || !matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
            )
        {
            return None;
        }
        let brace = self.frame_stack.active_brace().filter(|frame| {
            matches!(
                frame.semantic_kind,
                BraceSemanticKind::Array | BraceSemanticKind::Initializer
            )
        })?;
        if let Some(delimiter) = self
            .frame_stack
            .active_delimiter()
            .filter(|frame| frame.role == ParenRole::CastOrGroup)
        {
            return Some(delimiter.opener_output_column);
        }
        self.frame_stack
            .active_delimiter()
            .is_none()
            .then_some(brace.body_indent_column)
    }

    pub(super) fn initializer_or_array_closing_brace_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if line.trim() != "}" {
            return None;
        }
        let (open_spaces, _, open) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        if open.starts_with("},{") {
            return Some(open_spaces);
        }
        if !open.ends_with("{{") {
            return None;
        }
        if !open.trim_start().starts_with('.') {
            return Some(open_spaces + self.options.indent_width);
        }
        let previous_ends_comma = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|previous| {
                previous[..trailing_comment_split_limit(previous)]
                    .trim_end()
                    .ends_with(',')
            });
        Some(open_spaces + usize::from(previous_ends_comma) * self.options.indent_width)
    }

    pub(super) fn compound_literal_opening_layout(
        &self,
        line: &str,
        normal_indent: usize,
        indent: usize,
        exact_indent_spaces: Option<usize>,
    ) -> Option<CompoundLiteralOpeningLayout> {
        if self
            .frame_stack
            .active_brace()
            .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Command)
            || !line
                .trim_end()
                .strip_suffix('{')
                .is_some_and(|prefix| line_ends_compound_literal_cast(prefix.trim_end()))
        {
            return None;
        }
        let normal_spaces = normal_indent * self.options.indent_width;
        let call_argument_spaces = if line.trim_start().starts_with('(')
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|previous| {
                    let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                    code.ends_with(',') && unmatched_open_paren_column(code).is_none()
                }) {
            exact_indent_spaces.filter(|spaces| *spaces > normal_spaces)
        } else {
            None
        };
        let line_indent_spaces = if self.in_initializer_brace()
            || self.in_aggregate_declaration_brace()
        {
            if line.trim_start().starts_with('.') {
                let limit = self.token_input.input_source_indent.max(normal_spaces);
                self.output
                    .iter()
                    .rev()
                    .find(|line| {
                        line.trim_start().starts_with('.')
                            && leading_visual_width(line, self.options.tab_width) <= limit
                    })
                    .map(|previous| leading_visual_width(previous, self.options.tab_width))
                    .unwrap_or(self.token_input.input_source_indent)
            } else {
                self.output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .and_then(|previous| {
                        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                        code.ends_with('{').then(|| {
                            leading_visual_width(previous, self.options.tab_width)
                                + self.options.indent_width
                        })
                    })
                    .unwrap_or(normal_spaces)
            }
        } else if self.output.last_non_empty_line().is_some_and(|previous| {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            is_braceless_header_line(code.trim_start())
        }) {
            exact_indent_spaces.unwrap_or(indent * self.options.indent_width)
        } else {
            call_argument_spaces.unwrap_or(normal_spaces)
        };
        let brace_indent_spaces = if call_argument_spaces == Some(line_indent_spaces) {
            normal_spaces
        } else {
            line_indent_spaces
        };
        Some(CompoundLiteralOpeningLayout {
            line_indent_spaces,
            brace_indent_spaces,
        })
    }

    pub(super) fn initializer_brace_line_comment_gap(&self, brace_line: &str) -> String {
        if self.options.brace_style != BraceStyle::Horstmann {
            return "   ".to_string();
        }
        let brace_column = leading_visual_width(brace_line, self.options.tab_width);
        let target = brace_column + self.options.indent_width;
        super::brace_postprocess::horstmann_run_in_fill(
            brace_line,
            &" ".repeat(target),
            self.options,
        )
    }

    pub(super) fn open_expanded_init_brace(
        &mut self,
        brace_header: Option<String>,
        brace_type: FormatterBraceType,
        block_indent_extra: usize,
    ) {
        self.emit_source_space_or_ensure();
        self.current.push('{');
        self.command_state.observe_char('{');
        self.finish_line();
        self.stack_state
            .enter_brace(brace_header, brace_type, block_indent_extra);
        self.state.enter_block_with_extra(false, block_indent_extra);
        self.previous = PreviousToken::Other;
    }

    pub(super) fn open_multiline_attached_initializer_brace(
        &mut self,
        brace_header: Option<String>,
        brace_type: FormatterBraceType,
        block_indent_extra: usize,
        force_break_one_line: bool,
    ) {
        let control_paren_indent = self.control_paren_init_brace_indent_spaces();
        let brace_begins_line = self.current_is_blank();
        let enclosing_body_column = self.current_inline_array_column();
        let indented_initializer_brace = brace_begins_line
            && control_paren_indent.is_none()
            && enclosing_body_column.is_some()
            && matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
            );
        let opening_indent = control_paren_indent
            .or(enclosing_body_column)
            .unwrap_or_else(|| self.current_line_indent_spaces())
            + usize::from(indented_initializer_brace) * self.options.indent_width;
        let body_indent = if indented_initializer_brace {
            opening_indent
        } else {
            opening_indent + self.options.indent_width
        };

        self.emit_opening_brace_space(brace_type);
        self.current.push('{');
        self.command_state.observe_char('{');
        if brace_begins_line {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(opening_indent);
        }
        self.finish_line();
        self.update_current_brace_indent_from_last_output_line();
        self.stack_state
            .enter_brace(brace_header, brace_type, block_indent_extra);
        self.state.enter_block_without_indent(false);
        if force_break_one_line {
            self.compound_literal
                .forced_break_depths
                .push(self.stack_state.brace_header_stack.len());
        }
        let brace_column = if self.options.brace_style == BraceStyle::Ratliff
            && brace_type == FormatterBraceType::CompoundLiteral
        {
            opening_indent + self.options.indent_width
        } else {
            opening_indent
        };
        self.inline_array.frames.push(InlineArrayFrame {
            depth: self.stack_state.brace_header_stack.len(),
            body_column: body_indent,
            brace_column,
            output_line: self.output.len(),
            aggregate_assignment: control_paren_indent.is_some(),
        });
        self.continuation_indent.next_line_indent = None;
        self.continuation_indent.next_line_indent_spaces = Some(body_indent);
        self.previous = PreviousToken::Other;
    }

    pub(super) fn open_attached_range_for_init_brace(
        &mut self,
        brace_header: Option<String>,
        block_indent_extra: usize,
    ) {
        let opening_indent = self
            .for_header_continuation_indent_spaces()
            .unwrap_or_else(|| self.current_line_indent_spaces() + self.options.indent_width * 2);
        self.emit_source_space_or_ensure();
        self.current.push('{');
        self.command_state.observe_char('{');
        self.finish_line();
        self.stack_state
            .enter_brace(brace_header, FormatterBraceType::Array, block_indent_extra);
        self.state.enter_block_without_indent(false);
        self.inline_array.frames.push(InlineArrayFrame {
            depth: self.stack_state.brace_header_stack.len(),
            body_column: opening_indent + self.options.indent_width,
            brace_column: opening_indent,
            output_line: self.output.len().saturating_sub(1),
            aggregate_assignment: false,
        });
        self.update_current_brace_indent_columns(
            opening_indent + self.options.indent_width,
            opening_indent,
        );
        self.continuation_indent.next_line_indent = None;
        self.continuation_indent.next_line_indent_spaces =
            Some(opening_indent + self.options.indent_width);
        self.previous = PreviousToken::Other;
        self.previous_was_newline = true;
    }

    pub(super) fn open_range_for_init_brace(
        &mut self,
        _brace_header: Option<String>,
        _brace_type: FormatterBraceType,
        _block_indent_extra: usize,
    ) {
        let opening_indent = self
            .for_header_continuation_indent_spaces()
            .unwrap_or_else(|| self.current_line_indent_spaces() + self.options.indent_width * 2);
        self.finish_line();
        self.continuation_indent.next_line_indent = None;
        self.continuation_indent.next_line_indent_spaces = Some(opening_indent);
        self.current.push('{');
        self.command_state.observe_char('{');
        self.stack_state
            .enter_brace(None, FormatterBraceType::Array, 0);
        self.state.enter_block_without_indent(false);
        self.inline_array.frames.push(InlineArrayFrame {
            depth: self.stack_state.brace_header_stack.len(),
            body_column: opening_indent + 1,
            brace_column: opening_indent,
            output_line: self.output.len(),
            aggregate_assignment: true,
        });
        self.update_current_brace_indent_columns(opening_indent + 1, opening_indent);
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn open_inline_array_brace(
        &mut self,
        brace_header: Option<String>,
        brace_type: FormatterBraceType,
        block_indent_extra: usize,
        token_index: usize,
        first_is_brace: bool,
    ) {
        let enclosed = brace_type == FormatterBraceType::Array
            && matches!(
                self.stack_state.brace_type_stack.last(),
                Some(
                    FormatterBraceType::Array
                        | FormatterBraceType::Init
                        | FormatterBraceType::CompoundLiteral
                        | FormatterBraceType::Enum
                )
            );
        let double_brace_initializer = matches!(
            brace_type,
            FormatterBraceType::Array | FormatterBraceType::Init
        ) && self.current.trim_end().ends_with('{');
        let run_in_after_comma = enclosed
            && !self.token_input.token_begins_source_line
            && self.current.trim_end().ends_with(',');
        let range_for_header_initializer =
            double_brace_initializer && self.current.trim_start().starts_with("for (");
        let break_first = (enclosed || double_brace_initializer)
            && !first_is_brace
            && !run_in_after_comma
            && !range_for_header_initializer;
        let nested = enclosed
            || double_brace_initializer
            || (brace_type == FormatterBraceType::Array
                && self.inline_array.nested_brace_arrays.contains(&token_index));
        let constructor_indent = self
            .frame_stack
            .active_constructor_initializer()
            .map(|frame| frame.colon_line_indent_spaces);
        let base_indent = if constructor_indent.is_some() && self.stack_state.paren_depth == 0 {
            if self.current.trim_start().starts_with([':', ',']) {
                constructor_indent.unwrap_or_else(|| self.current_line_indent_spaces())
            } else {
                self.constructor_initializer_base_indent_spaces()
                    .unwrap_or_else(|| self.current_line_indent_spaces())
            }
        } else {
            self.current_line_indent_spaces()
                .max(constructor_indent.unwrap_or(0))
        };
        let aggregate_assign = self.current.trim_end().ends_with('=');
        if self.current_is_lambda_body_header() || is_lambda_capture_header(self.current.trim_end())
        {
            self.emit_source_space_or_ensure();
        } else {
            match self.current.trim_end().chars().next_back() {
                Some('[') => self.emit_source_space_or_ensure(),
                Some('(') if self.options.pad_parens_inside => {
                    self.pad_inside_paren_space();
                }
                Some('(') => self.emit_source_space(),
                Some('{') if self.current.ends_with([' ', '\t']) => {}
                Some('@') => self.emit_source_space_or_ensure(),
                _ if brace_type == FormatterBraceType::Init
                    && self.current.trim_end().ends_with('>') =>
                {
                    self.emit_source_space_or_ensure();
                }
                _ if aggregate_assign => self.emit_source_space_or_ensure(),
                _ => self.emit_source_space(),
            }
        }
        self.current.push('{');
        self.command_state.observe_char('{');
        let brace_column = if nested {
            if run_in_after_comma && !first_is_brace {
                base_indent + self.options.indent_width
            } else {
                base_indent
            }
        } else {
            base_indent + self.current_char_len() - 1
        };
        if !break_first {
            self.emit_trailing_source_space();
        }
        let mut column = if nested {
            if double_brace_initializer {
                base_indent + self.options.indent_width * 2
            } else {
                base_indent + self.options.indent_width
            }
        } else {
            base_indent + self.current_char_len()
        };
        let statement_base = super::ContinuationIndent::Level(
            self.state.line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal),
        )
        .columns(self.options.indent_width);
        if brace_type == FormatterBraceType::Init
            && line_opens_typed_initializer(&self.current)
            && column.saturating_sub(statement_base) > self.options.max_continuation_indent
        {
            column = base_indent + self.options.indent_width * 2;
        }
        let current_trimmed = self.current.trim_start();
        let run_in_nested_brace = current_trimmed.starts_with("{{") || self.current.contains("{ {");
        let stored_brace_column = if self.current.trim() == "{" {
            column
        } else if run_in_nested_brace {
            base_indent + self.options.indent_width
        } else {
            brace_column
        };
        self.stack_state
            .enter_brace(brace_header, brace_type, block_indent_extra);
        self.state.enter_block_without_indent(false);
        self.inline_array.frames.push(InlineArrayFrame {
            depth: self.stack_state.brace_header_stack.len(),
            body_column: column,
            brace_column: stored_brace_column,
            output_line: self.output.len(),
            aggregate_assignment: aggregate_assign,
        });
        if self.current.trim() == "{" {
            self.update_current_brace_indent_columns(column + self.options.indent_width, column);
        } else if run_in_nested_brace {
            self.update_current_brace_indent_columns(
                base_indent + self.options.indent_width * 2,
                base_indent + self.options.indent_width,
            );
        } else {
            self.update_current_brace_indent_columns(
                base_indent + self.options.indent_width,
                base_indent,
            );
        }
        if break_first {
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(column);
        }
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn close_inline_array_brace(&mut self) {
        if self.current_is_blank() {
            self.frame_stack.clear_closed_braces();
        }
        let closing_brace_type = self.stack_state.brace_type_stack.last().copied();
        let closing_compound_literal = matches!(
            closing_brace_type,
            Some(FormatterBraceType::CompoundLiteral)
        );
        let closing_enum = matches!(closing_brace_type, Some(FormatterBraceType::Enum));
        let inline_array = self.inline_array.frames.pop();
        let body_column = inline_array.map(|frame| frame.body_column);
        let in_constructor_initializer =
            self.frame_stack.active_constructor_initializer().is_some();
        self.inline_array.current_closed_body_column =
            body_column.map(|column| (column, in_constructor_initializer));
        let brace_column = inline_array.map(|frame| frame.brace_column);
        let open_output_len = inline_array
            .map(|frame| frame.output_line)
            .unwrap_or(self.output.len());
        let aggregate_assign = inline_array.is_some_and(|frame| frame.aggregate_assignment);
        let objc_dictionary = self
            .output
            .get(open_output_len)
            .is_some_and(|line| line.contains("@ {"));
        let return_initializer = self
            .output
            .get(open_output_len)
            .is_some_and(|line| line.trim_start().starts_with("return "));
        let enclosed_run_in = self
            .output
            .get(open_output_len)
            .is_some_and(|line| line.contains("{ {"));
        let range_for_initializer = self.output.get(open_output_len).is_some_and(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("for ") && trimmed.trim_end().ends_with('{')
        });
        let call_argument_array_column = self.output.get(open_output_len).and_then(|line| {
            line.rfind(", {")
                .map(|comma| visual_width_from(&line[..comma + 2], 0, self.options.tab_width))
        });
        let call_argument_array = call_argument_array_column.is_some();
        let typed_initializer = matches!(closing_brace_type, Some(FormatterBraceType::Init))
            && self
                .output
                .get(open_output_len)
                .is_some_and(|line| line_opens_typed_initializer(line));
        let closing_column = call_argument_array_column.or(brace_column);
        let forced_break = self
            .compound_literal
            .forced_break_depths
            .last()
            .is_some_and(|depth| *depth == self.stack_state.brace_header_stack.len());
        if forced_break {
            self.compound_literal.forced_break_depths.pop();
        }
        self.exit_brace_state();
        if let Some(frame) = self.frame_stack.last_closed_brace_mut() {
            if in_constructor_initializer && let Some(column) = body_column {
                frame.body_indent_column = column;
            }
            if let Some(column) = closing_column {
                frame.sibling_indent_column = column;
            }
        }
        if forced_break {
            if !self.current_is_blank() {
                self.finish_line();
            }
            if let Some(column) = closing_column {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(column);
            }
            self.trim_current_end();
            self.mark_closed_brace_output_position();
            self.current.push('}');
            self.command_state.observe_char('}');
            self.compound_literal.just_closed = closing_compound_literal;
            self.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return;
        }
        let parameterized_lambda_initializer_close = self.current_is_blank()
            && self.output.last().is_some_and(|line| line.trim() == "}")
            && self.output.get(open_output_len).is_some_and(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.contains("](") || code.contains("] (")
            });
        if parameterized_lambda_initializer_close {
            if let Some(previous) = self.output.pop() {
                self.current.replace(previous);
            }
        } else if self.token_input.token_begins_source_line {
            if !self.current_is_blank() {
                self.finish_line();
            }
            if let Some(column) = closing_column {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(column);
            }
        } else if self.output.len() > open_output_len
            && !self.current_is_blank()
            && (closing_compound_literal
                || aggregate_assign
                || objc_dictionary
                || closing_enum
                || enclosed_run_in
                || range_for_initializer
                || call_argument_array
                || typed_initializer)
        {
            self.finish_line();
            if let Some(column) = closing_column {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(column);
            }
        }
        let return_initializer_gap = return_initializer.then(|| {
            self.token_input
                .previous_input_whitespace
                .clone()
                .filter(|gap| !gap.is_empty() && gap.chars().all(|ch| ch == ' ' || ch == '\t'))
        });
        let source_closing_gap = (!closing_compound_literal
            && !aggregate_assign
            && !range_for_initializer
            && !call_argument_array)
            .then(|| {
                self.token_input
                    .previous_input_whitespace
                    .clone()
                    .filter(|gap| !gap.is_empty() && gap.chars().all(|ch| ch == ' ' || ch == '\t'))
            });
        if let Some(Some(gap)) = return_initializer_gap.or(source_closing_gap) {
            self.trim_current_end_horizontal_space();
            self.current.push_str(&gap);
        } else {
            self.trim_current_end_horizontal_space();
        }
        self.mark_closed_brace_output_position();
        self.current.push('}');
        self.command_state.observe_char('}');
        self.compound_literal.just_closed = closing_compound_literal;
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn current_inline_array_column(&self) -> Option<usize> {
        self.inline_array
            .frames
            .last()
            .filter(|frame| frame.depth == self.stack_state.brace_header_stack.len())
            .map(|frame| frame.body_column)
    }

    pub(super) fn active_initializer_brace_indent_spaces(
        &self,
        line: &str,
        closing: bool,
    ) -> Option<usize> {
        let starts_member_opener =
            !closing && line.trim_start().starts_with('.') && line.contains('{');
        let frame = if closing {
            let trimmed = line.trim_start();
            let previous_closes_brace = self
                .output
                .last()
                .is_some_and(|line| line.trim_start().starts_with('}'));
            let closed = if previous_closes_brace
                || trimmed.starts_with("},")
                || trimmed.starts_with("};")
                || trimmed.starts_with("})")
            {
                self.frame_stack.last_closed_brace()
            } else {
                self.frame_stack
                    .first_closed_brace()
                    .or_else(|| self.frame_stack.last_closed_brace())
            };
            closed.or_else(|| self.frame_stack.active_brace())?
        } else if starts_member_opener {
            let opened_braces = line.chars().filter(|ch| *ch == '{').count();
            self.frame_stack
                .brace_before_top(opened_braces)
                .or_else(|| self.frame_stack.enclosing_brace())
                .or_else(|| self.frame_stack.active_brace())?
        } else {
            self.frame_stack.active_brace()?
        };
        if !matches!(
            frame.semantic_kind,
            BraceSemanticKind::Array
                | BraceSemanticKind::CompoundLiteral
                | BraceSemanticKind::Initializer
        ) {
            return None;
        }
        Some(if closing {
            frame.sibling_indent_column
        } else {
            frame.body_indent_column
        })
    }

    pub(super) fn initializer_member_indent_spaces(&self, line: &str) -> Option<usize> {
        if !self.in_initializer_brace() {
            return None;
        }
        let trimmed = line.trim_start();
        let closing = trimmed.starts_with('}');
        let designator = trimmed.starts_with('.') || trimmed.starts_with('[');
        if !closing && !designator {
            return None;
        }
        if closing && let Some(spaces) = self.continuation_indent.next_line_indent_spaces {
            return Some(spaces);
        }
        if !closing
            && let Some(previous) = self.output.last()
            && previous.trim_end().ends_with("},{")
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        if !closing
            && designator
            && let Some(previous) = self.output.last()
            && previous.trim_start().starts_with("},")
        {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        if !closing
            && let Some(previous) = self.output.last()
            && (previous.contains("{{") || previous.contains("{ {"))
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width * 2,
            );
        }
        if !closing
            && designator
            && let Some(previous) = self.output.last()
            && previous.trim_end().ends_with('{')
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        if closing
            && let Some(previous) = self.output.last()
            && previous.trim_start().starts_with('.')
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width),
            );
        }
        if !closing
            && let Some(mut spaces) = self.active_initializer_brace_indent_spaces(line, closing)
        {
            if designator
                && let Some(previous) = self.output.last()
                && previous.trim_start().starts_with(['.', '['])
            {
                spaces = leading_visual_width(previous, self.options.tab_width);
            }
            if designator {
                if self.output_has_open_initializer_brace() || self.in_initializer_brace() {
                    spaces = spaces.max(self.state.indent() * self.options.indent_width);
                }
                for (index, previous) in self.output.iter().enumerate().rev() {
                    let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                    if code.ends_with('{') && self.output_line_opens_initializer(index, code) {
                        spaces = spaces.max(
                            leading_visual_width(previous, self.options.tab_width)
                                + self.options.indent_width,
                        );
                        break;
                    }
                    if code.ends_with(';') || code.ends_with('}') {
                        break;
                    }
                }
            }
            return Some(spaces);
        }
        if closing
            && !matches!(
                self.stack_state.last_closed_brace_type,
                Some(
                    FormatterBraceType::Array
                        | FormatterBraceType::CompoundLiteral
                        | FormatterBraceType::Init
                )
            )
        {
            return None;
        }
        let mut depth = 0usize;
        for previous in self.output.iter().rev() {
            let trimmed_previous = previous.trim_end();
            let mut chars = trimmed_previous.chars().rev();
            while let Some(ch) = chars.next() {
                match ch {
                    '}' => depth += 1,
                    '{' if depth == 0 => {
                        let mut levels = 1usize;
                        for left in chars.by_ref() {
                            if left == '{' {
                                levels += 1;
                            } else if left.is_whitespace() {
                                continue;
                            } else {
                                break;
                            }
                        }
                        let prefix_len = leading_visual_width(previous, self.options.tab_width);
                        let inner_levels = levels - usize::from(closing);
                        return Some(prefix_len + inner_levels * self.options.indent_width);
                    }
                    '{' => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        None
    }

    pub(super) fn initializer_brace_continuation_anchor(&self, line: &str) -> Option<usize> {
        let head = line.trim_end().strip_suffix('{')?.trim_end();
        if !head.starts_with('=') || head.as_bytes().get(1) == Some(&b'=') {
            return None;
        }
        if self.stack_state.paren_depth > 0 {
            return None;
        }
        let frame = self.frame_stack.active_brace()?;
        if !matches!(
            frame.semantic_kind,
            BraceSemanticKind::Array
                | BraceSemanticKind::Initializer
                | BraceSemanticKind::CompoundLiteral
        ) {
            return None;
        }
        Some(self.continuation_base_indent() * self.options.indent_width)
    }

    pub(super) fn output_has_open_initializer_brace(&self) -> bool {
        for index in (0..self.output.len()).rev().take(16) {
            let code = self.output.code(index);
            let trimmed = code.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if code.ends_with(';') || trimmed == "{" || trimmed == "}" {
                return false;
            }
            if has_unmatched_open_brace(code) {
                return code.contains("({") || code.contains("= {") || code.contains("{{");
            }
        }
        false
    }

    pub(super) fn compound_initializer_value_indent(&self, trimmed: &str) -> Option<usize> {
        if trimmed.starts_with(['}', ')', '.', '[']) || starts_ternary_arm(trimmed) {
            return None;
        }
        let (index, previous) = self
            .output
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with('{')
            || !self.output_line_opens_initializer(index, previous_code)
        {
            return None;
        }
        let double_brace_extra =
            usize::from(previous_code.ends_with("{{")) * self.options.indent_width;
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + self.options.indent_width
                + double_brace_extra,
        )
    }

    pub(super) fn initializer_current_indent_matches_previous_row(
        &self,
        trimmed: &str,
        current_spaces: usize,
        source: usize,
    ) -> bool {
        if source >= current_spaces
            || trimmed.starts_with('.')
            || trimmed.starts_with('[')
            || trimmed.contains('(')
            || starts_ternary_arm(trimmed)
            || starts_with_chain_operator(trimmed)
            || !self.initializer_line_keeps_source_indent(trimmed)
            || !(self.in_initializer_brace()
                || self.innermost_init_block_brace()
                || self.in_aggregate_declaration_brace()
                || self.current_inline_array_column().is_some()
                || self.output_has_open_initializer_brace()
                || self.previous_comma_inside_open_brace())
        {
            return false;
        }
        let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        else {
            return false;
        };
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        previous_code.ends_with(',')
            && leading_visual_width(previous, self.options.tab_width) >= current_spaces
    }

    pub(super) fn initializer_line_keeps_source_indent(&self, trimmed: &str) -> bool {
        if trimmed.starts_with("};") || trimmed.starts_with("];") {
            return false;
        }
        if trimmed.starts_with(['.', '[', '{']) || trimmed.starts_with("},") {
            return true;
        }
        let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        else {
            return false;
        };
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        previous_code.ends_with(',') && !trimmed.starts_with(['}', ')'])
    }

    pub(super) fn previous_comma_inside_open_brace(&self) -> bool {
        self.previous_initializer_comma_indent().is_some()
    }

    pub(super) fn previous_initializer_comma_indent(&self) -> Option<usize> {
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        let mut closed = 0usize;
        for index in (0..self.output.len()).rev().take(64) {
            let line = &self.output[index];
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim();
            if trimmed.ends_with(';') {
                return None;
            }
            for ch in code.chars().rev() {
                match ch {
                    '}' => closed += 1,
                    '{' if closed > 0 => closed -= 1,
                    '{' if self.output_line_opens_initializer(index, code) => {
                        return Some(leading_visual_width(previous, self.options.tab_width));
                    }
                    _ => {}
                }
            }
        }
        None
    }

    pub(super) fn preprocessor_branch_initializer_member_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if line.trim_start().starts_with('#')
            || !(self.current_inline_array_column().is_some()
                || self.in_initializer_brace()
                || self.in_aggregate_declaration_brace())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if !preprocessor_directive(previous.trim_start()).is_some_and(is_conditional_preprocessor) {
            return None;
        }
        let row = self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })?;
        let row_code = row[..trailing_comment_split_limit(row)].trim_end();
        row_code
            .ends_with(',')
            .then(|| leading_visual_width(row, self.options.tab_width))
    }

    pub(super) fn split_else_initializer_closing_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
        case_unindent_spaces: usize,
    ) -> Option<usize> {
        if !split_else_context || case_unindent_spaces == 0 || !line.trim_start().starts_with("};")
        {
            return None;
        }
        self.active_initializer_brace_indent_spaces(line, true)
            .map(|spaces| spaces + case_unindent_spaces)
    }

    pub(super) fn split_else_commented_aggregate_member_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        split_else_extra_indent: bool,
        case_unindent_spaces: usize,
    ) -> Option<usize> {
        if !split_else_extra_indent
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}'])
            || case_unindent_spaces == 0
            || !self.in_aggregate_declaration_brace()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(';')
            || !(previous_code.len() < previous.trim_end().len() || previous_code.contains("/*"))
        {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width) + case_unindent_spaces)
    }

    pub(super) fn aggregate_member_case_indent_spaces(
        &self,
        current_spaces: usize,
        normal_indent: usize,
        case_unindent_spaces: usize,
    ) -> Option<usize> {
        if case_unindent_spaces == 0 || current_spaces > normal_indent * self.options.indent_width {
            return None;
        }
        let aggregate_member = self.in_aggregate_declaration_brace()
            || self.output.iter().rev().take(16).any(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.trim_start().starts_with("static const struct") && code.ends_with('{')
            });
        aggregate_member.then_some(current_spaces + case_unindent_spaces)
    }

    pub(super) fn output_line_opens_initializer(&self, index: usize, code: &str) -> bool {
        let trimmed = code.trim();
        if code.contains("= {") || code.contains("({") || code.contains("{{") {
            return true;
        }
        if trimmed.ends_with('{') {
            let head = trimmed.trim_end_matches('{').trim_end();
            if line_ends_compound_literal_cast(head) {
                return true;
            }
        }
        if trimmed != "{" {
            return false;
        }
        self.output[..index]
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|previous| {
                let previous = previous[..trailing_comment_split_limit(previous)].trim_end();
                previous.contains('=') && previous.ends_with(')')
            })
    }
}
